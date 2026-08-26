from contextlib import AbstractContextManager, nullcontext
from functools import lru_cache
from typing import TypeVar

import torch
from torch import nn

import env

Model = TypeVar("Model", bound=nn.Module)


@lru_cache(maxsize=1)
def get_device() -> torch.device:
    device = torch.device(env.pneuma_device())
    if device.type == "cuda" and not torch.cuda.is_available():
        raise RuntimeError(f"Pneuma requested unavailable device {device}")
    return device


def use_half_precision() -> bool:
    return get_device().type == "cuda"


def prepare_model(model: Model) -> Model:
    return model.to(get_device()).eval()


def autocast() -> (
    AbstractContextManager[torch.autocast] | AbstractContextManager[None]
):
    if use_half_precision():
        return torch.autocast(device_type="cuda", dtype=torch.float16)
    return nullcontext()


def empty_cache() -> None:
    if get_device().type == "cuda":
        torch.cuda.empty_cache()


def is_cuda_out_of_memory_error(error: BaseException) -> bool:
    return isinstance(error, torch.OutOfMemoryError) or "CUDA out of memory" in str(
        error
    )
