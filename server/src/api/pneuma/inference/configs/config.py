import logging
from dataclasses import dataclass
from functools import lru_cache
from multiprocessing import cpu_count

import torch

from api.pneuma.inference.device import get_device, use_half_precision

logger = logging.getLogger("shrimply.pneuma")


@dataclass(frozen=True, slots=True)
class ConfigData:
    n_cpu: int
    gpu_name: str | None
    gpu_mem: int | None

    preprocess_segment_seconds: float
    x_pad: int
    x_query: int
    x_center: int
    x_max: int


def device_metadata(device: torch.device) -> tuple[str | None, int | None]:
    if device.type == "cpu":
        return None, None
    if device.type != "cuda":
        return device.type, None
    device_index = (
        device.index if device.index is not None else torch.cuda.current_device()
    )
    properties = torch.cuda.get_device_properties(device_index)
    gpu_mem = int(properties.total_memory / 1024**3)
    return properties.name, gpu_mem


def vc_chunk_config(is_half: bool, gpu_mem: int | None) -> tuple[int, int, int, int]:
    if gpu_mem is not None and gpu_mem <= 4:
        return 1, 5, 30, 32
    if is_half:
        # VRAM >= 6GB: use x_pad=3, x_query=10, x_center=60, x_max=65
        return 3, 10, 60, 65
    # VRAM >= 4GB: use x_pad=1, x_query=6, x_center=38, x_max=41
    return 1, 6, 38, 41


@lru_cache(maxsize=1)
def get_config() -> ConfigData:
    preprocess_segment_seconds: float = 3.7
    n_cpu = cpu_count()
    device = get_device()
    gpu_name, gpu_mem = device_metadata(device)
    logger.info(f"Using device {device}")
    is_half = use_half_precision()
    x_pad, x_query, x_center, x_max = vc_chunk_config(is_half, gpu_mem)
    logger.info(f"Half-precision floating-point: {is_half}, device: {device}")

    return ConfigData(
        n_cpu=n_cpu,
        gpu_name=gpu_name,
        gpu_mem=gpu_mem,
        preprocess_segment_seconds=preprocess_segment_seconds,
        x_pad=x_pad,
        x_query=x_query,
        x_center=x_center,
        x_max=x_max,
    )
