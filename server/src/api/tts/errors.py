import logging
import traceback

import torch


_MAX_PUBLIC_ERROR_LENGTH = 512
_CUDA_OUT_OF_MEMORY_MESSAGE = (
    "Not enough GPU memory to generate speech. "
    "Try a smaller model or lower precision."
)


def public_error_message(exception: Exception) -> str:
    message = str(exception).strip()
    if isinstance(exception, torch.OutOfMemoryError) or "CUDA out of memory" in message:
        return _CUDA_OUT_OF_MEMORY_MESSAGE
    if (
        not message
        or len(message) > _MAX_PUBLIC_ERROR_LENGTH
        or not message.isprintable()
    ):
        return f"{type(exception).__name__} details were omitted"
    return message


def log_failure(
    logger: logging.Logger,
    message: str,
    exception: Exception,
) -> None:
    frames = traceback.extract_tb(exception.__traceback__)
    if not frames:
        logger.error("%s error_type=%s", message, type(exception).__name__)
        return
    frame = frames[-1]
    logger.error(
        "%s error_type=%s source=%s:%d function=%s",
        message,
        type(exception).__name__,
        frame.filename,
        frame.lineno,
        frame.name,
    )
