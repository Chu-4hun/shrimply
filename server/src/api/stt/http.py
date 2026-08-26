import logging
import os
import tempfile
from http import HTTPStatus

from fastapi import Request
from fastapi.responses import Response
from pydantic import ValidationError

from api import log_request, request_content_type, resource, respond_error, stream_events
from api import stt
from api.stt.protocol import MODEL_ID_VALIDATOR, ErrorEvent
from api.stt.resources import MAXIMUM_AUDIO_BYTES

logger = logging.getLogger("shrimply.server")


async def handle_request(request: Request) -> Response:
    log_request(request, "speech-to-text")
    try:
        job = resource.register_job(request.headers.get(resource.JOB_HEADER))
    except resource.InvalidJobId as exception:
        return respond_error(HTTPStatus.BAD_REQUEST, "invalid_job_id", str(exception))
    except resource.DuplicateJobId as exception:
        return respond_error(HTTPStatus.CONFLICT, "duplicate_job_id", str(exception))
    except resource.JobCancelled as exception:
        return respond_error(HTTPStatus.CONFLICT, exception.code, str(exception))
    model_values = request.query_params.getlist("model")
    if len(model_values) != 1:
        resource.finish_job(job)
        return respond_error(
            HTTPStatus.BAD_REQUEST,
            "model_required",
            "Exactly one speech-to-text model is required",
        )
    try:
        model_id = MODEL_ID_VALIDATOR.validate_python(model_values[0])
    except ValidationError:
        resource.finish_job(job)
        return respond_error(
            HTTPStatus.BAD_REQUEST,
            "unsupported_model",
            f"Unsupported speech-to-text model: {model_values[0]}",
        )
    if request_content_type(request) != "application/octet-stream":
        resource.finish_job(job)
        return respond_error(
            HTTPStatus.UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "Expected application/octet-stream",
        )
    try:
        content_length = int(request.headers.get("content-length", ""))
    except ValueError:
        resource.finish_job(job)
        return respond_error(
            HTTPStatus.LENGTH_REQUIRED,
            "content_length_required",
            "A valid Content-Length header is required",
        )
    if content_length <= 0 or content_length > MAXIMUM_AUDIO_BYTES:
        resource.finish_job(job)
        return respond_error(
            HTTPStatus.BAD_REQUEST,
            "invalid_content_length",
            "Audio request must be between 1 byte and 512 MiB",
        )
    path: str | None = None
    try:
        with tempfile.NamedTemporaryFile(prefix="shrimply-stt-", delete=False) as audio:
            path = audio.name
            remaining = content_length
            async for chunk in request.stream():
                job.check_cancelled()
                if len(chunk) > remaining:
                    raise ValueError("Audio body exceeds Content-Length")
                audio.write(chunk)
                remaining -= len(chunk)
            if remaining:
                raise ValueError("Audio length does not match Content-Length")
    except resource.JobCancelled as exception:
        if path is not None:
            os.unlink(path)
        resource.finish_job(job)
        return respond_error(HTTPStatus.CONFLICT, exception.code, str(exception))
    except ValueError as exception:
        if path is not None:
            os.unlink(path)
        resource.finish_job(job)
        return respond_error(HTTPStatus.BAD_REQUEST, "invalid_content_length", str(exception))
    except Exception:
        if path is not None:
            os.unlink(path)
        resource.finish_job(job)
        raise
    assert path is not None
    if content_length % 4 != 0:
        os.unlink(path)
        resource.finish_job(job)
        return respond_error(
            HTTPStatus.BAD_REQUEST,
            "invalid_audio",
            "Audio length is not a multiple of one little-endian f32 sample",
        )
    client = request.client.host if request.client is not None else "unknown"
    logger.info(
        "Accepted speech-to-text request from %s model=%s with %d audio bytes",
        client,
        model_id,
        content_length,
    )

    def failure(exception: Exception) -> ErrorEvent:
        try:
            job.check_cancelled()
        except resource.JobCancelled as cancellation:
            exception = cancellation
        if isinstance(exception, resource.JobCancelled):
            return ErrorEvent(code=exception.code, message=str(exception))
        logger.exception("Speech-to-text request failed")
        return ErrorEvent(code="worker_failed", message=str(exception))

    def transcribe(send_event):
        stt.transcribe(model_id, path, content_length, job, send_event)

    return stream_events(
        transcribe, failure, "stt-request", job, lambda: os.unlink(path)
    )
