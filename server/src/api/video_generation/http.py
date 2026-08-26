import logging
from functools import partial
from http import HTTPStatus

import msgspec
from fastapi import Request
from fastapi.responses import Response
from pydantic import ValidationError

from api import gpu, log_request, request_content_type, respond_error, stream_events
from api import resource
from api.tts.errors import log_failure, public_error_message
from api.video_generation import generate
from api.video_generation.protocol import ErrorEvent, GENERATION_REQUEST_VALIDATOR
from api.video_generation.requests import validate_inputs

logger = logging.getLogger("shrimply.server")
MAXIMUM_REQUEST_BYTES = 513 * 1024 * 1024


async def handle_request(request: Request) -> Response:
    log_request(request, "video generation")
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

    if request_content_type(request) != "application/msgpack":
        return reject(
            respond_error(
                HTTPStatus.UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                "Expected application/msgpack",
            )
        )
    try:
        content_length = int(request.headers.get("content-length", ""))
    except ValueError:
        return reject(
            respond_error(
                HTTPStatus.LENGTH_REQUIRED,
                "content_length_required",
                "A valid Content-Length header is required",
            )
        )
    if content_length <= 0 or content_length > MAXIMUM_REQUEST_BYTES:
        return reject(
            respond_error(
                HTTPStatus.BAD_REQUEST,
                "invalid_content_length",
                "Video-generation request must be between 1 byte and 513 MiB",
            )
        )
    if not gpu.device.startswith("cuda:"):
        return reject(
            respond_error(
                HTTPStatus.SERVICE_UNAVAILABLE,
                "cuda_required",
                "Video generation requires a selected CUDA device",
            )
        )
    try:
        received = bytearray()
        async for chunk in request.stream():
            job.check_cancelled()
            if len(chunk) > content_length - len(received):
                raise ValueError("Video-generation request body exceeds Content-Length")
            received.extend(chunk)
        payload = bytes(received)
    except resource.JobCancelled as exception:
        return reject(
            respond_error(HTTPStatus.CONFLICT, exception.code, str(exception))
        )
    except ValueError as exception:
        return reject(
            respond_error(HTTPStatus.BAD_REQUEST, "invalid_content_length", str(exception))
        )
    except Exception:
        resource.finish_job(job)
        raise
    if len(payload) != content_length:
        return reject(
            respond_error(
                HTTPStatus.BAD_REQUEST,
                "invalid_content_length",
                "Video-generation request length does not match Content-Length",
            )
        )
    try:
        generation_request = GENERATION_REQUEST_VALIDATOR.validate_python(
            msgspec.msgpack.decode(payload)
        )
        validate_inputs(generation_request)
    except (msgspec.DecodeError, ValidationError, ValueError) as exception:
        return reject(
            respond_error(HTTPStatus.BAD_REQUEST, "invalid_request", str(exception))
        )

    client = request.client.host if request.client is not None else "unknown"
    logger.info(
        "Accepted video-generation request from %s model=%s",
        client,
        generation_request.model,
    )

    def failure(exception: Exception) -> ErrorEvent:
        try:
            job.check_cancelled()
        except resource.JobCancelled as cancellation:
            exception = cancellation
        if isinstance(exception, resource.JobCancelled):
            return ErrorEvent(code=exception.code, message=str(exception))
        log_failure(logger, "Video-generation request failed", exception)
        return ErrorEvent(code="worker_failed", message=public_error_message(exception))

    return stream_events(
        partial(generate, job, generation_request),
        failure,
        "video-generation-request",
        job,
    )
