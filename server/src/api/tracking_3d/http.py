import asyncio
import logging
import os
import tempfile
from http import HTTPStatus

import msgspec
from fastapi import Request
from fastapi.responses import Response
from pydantic import ValidationError

from api import log_request, request_content_type, respond_error, stream_events
from api import resource, tracking_3d
from api.tracking_3d.protocol import (
    ANALYSIS_REQUEST_VALIDATOR,
    ARCHIVE_MAGIC,
    MAXIMUM_ARCHIVE_BYTES,
    MAXIMUM_HEADER_BYTES,
    MAXIMUM_JPEG_BYTES,
    AnalysisRequest,
    ErrorEvent,
)

logger = logging.getLogger("shrimply.server")
CONTENT_TYPE = "application/x-shrimply-3dtracking-analysis"


def read_header(path: str, job: resource.ManagedJob) -> AnalysisRequest:
    with open(path, "rb") as file:
        if file.read(len(ARCHIVE_MAGIC)) != ARCHIVE_MAGIC:
            raise ValueError("Invalid 3D tracking archive magic")
        header_length = int.from_bytes(file.read(8), "little")
        if header_length <= 0 or header_length > MAXIMUM_HEADER_BYTES:
            raise ValueError("Invalid 3D tracking archive header length")
        payload = file.read(header_length)
        if len(payload) != header_length:
            raise ValueError("Truncated 3D tracking archive header")
        request = ANALYSIS_REQUEST_VALIDATOR.validate_python(
            msgspec.msgpack.decode(payload)
        )
        size = os.path.getsize(path)
        seen: set[int] = set()
        for _ in range(request.frame_count):
            job.check_cancelled()
            record = file.read(16)
            if len(record) != 16:
                raise ValueError("Truncated 3D tracking archive")
            frame_index = int.from_bytes(record[:8], "little")
            length = int.from_bytes(record[8:], "little")
            if frame_index >= request.frame_count or frame_index in seen:
                raise ValueError("Invalid or duplicate 3D tracking frame index")
            if length > MAXIMUM_JPEG_BYTES or length > size - file.tell():
                raise ValueError("Invalid 3D tracking JPEG length")
            seen.add(frame_index)
            file.seek(length, 1)
        if file.tell() != size:
            raise ValueError("Unexpected trailing 3D tracking archive data")
        return request


async def handle_request(request: Request) -> Response:
    log_request(request, "3D tracking analysis")
    try:
        job = resource.register_job(request.headers.get(resource.JOB_HEADER))
    except resource.InvalidJobId as exception:
        return respond_error(HTTPStatus.BAD_REQUEST, "invalid_job_id", str(exception))
    except resource.DuplicateJobId as exception:
        return respond_error(HTTPStatus.CONFLICT, "duplicate_job_id", str(exception))
    except resource.JobCancelled as exception:
        return respond_error(HTTPStatus.CONFLICT, exception.code, str(exception))

    def reject(response: Response) -> Response:
        resource.finish_job(job)
        return response

    if request_content_type(request) != CONTENT_TYPE:
        return reject(
            respond_error(
                HTTPStatus.UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                f"Expected {CONTENT_TYPE}",
            )
        )
    try:
        content_length = int(request.headers.get("content-length", ""))
    except ValueError:
        content_length = 0
    if content_length <= 0 or content_length > MAXIMUM_ARCHIVE_BYTES:
        return reject(
            respond_error(
                HTTPStatus.BAD_REQUEST,
                "invalid_content_length",
                "3D tracking request must be between 1 byte and 8 GiB",
            )
        )
    try:
        resource.scheduler.reserve_temporary(job, content_length * 2)
    except resource.TemporaryStorageUnavailable as exception:
        return reject(
            respond_error(
                HTTPStatus.INSUFFICIENT_STORAGE,
                "temporary_storage_unavailable",
                str(exception),
            )
        )
    path: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            prefix="shrimply-3dtracking-", delete=False
        ) as temporary:
            path = temporary.name
            remaining = content_length
            async for chunk in request.stream():
                job.check_cancelled()
                if len(chunk) > remaining:
                    raise ValueError("3D tracking request body exceeds Content-Length")
                temporary.write(chunk)
                remaining -= len(chunk)
            job.check_cancelled()
            if remaining:
                raise ValueError("Truncated 3D tracking request body")
        analysis_request = await asyncio.to_thread(read_header, path, job)
    except (ValueError, msgspec.DecodeError, ValidationError) as exception:
        if path is not None:
            os.unlink(path)
        return reject(
            respond_error(HTTPStatus.BAD_REQUEST, "invalid_request", str(exception))
        )
    except resource.JobCancelled as exception:
        if path is not None:
            os.unlink(path)
        return reject(
            respond_error(HTTPStatus.CONFLICT, exception.code, str(exception))
        )
    except Exception:
        if path is not None:
            os.unlink(path)
        resource.finish_job(job)
        raise
    assert path is not None
    if (
        analysis_request.model == "MIT-SPARK/VGGT-SLAM"
        and not tracking_3d.cuda_selected()
    ):
        os.unlink(path)
        return reject(
            respond_error(
                HTTPStatus.BAD_REQUEST,
                "unsupported_device",
                "MIT-SPARK/VGGT-SLAM requires a CUDA compute device",
            )
        )
    client = request.client.host if request.client is not None else "unknown"
    logger.info(
        "Accepted 3D tracking request from %s job_id=%s model=%s with %d archive bytes",
        client,
        job.id,
        analysis_request.model,
        content_length,
    )

    def analyze(send_event):
        tracking_3d.analyze(
            job,
            analysis_request.model,
            path,
            send_event,
        )

    def failure(exception: Exception) -> ErrorEvent:
        try:
            job.check_cancelled()
        except resource.JobCancelled as cancellation:
            exception = cancellation
        if isinstance(exception, resource.JobCancelled):
            return ErrorEvent(code=exception.code, message=str(exception))
        logger.exception("3D tracking request failed")
        return ErrorEvent(code="worker_failed", message=str(exception))

    return stream_events(
        analyze, failure, "3dtracking-request", job, lambda: os.unlink(path)
    )
