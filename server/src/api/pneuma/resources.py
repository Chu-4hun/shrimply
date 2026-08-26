import math
from pathlib import Path

from safetensors import SafetensorError, safe_open

from api.resource import ResourceProfile

GIB = 1024**3
RUNTIME_WEIGHTS = 2 * GIB
INFERENCE_WORKSPACE = GIB
MAXIMUM_DECODED_AUDIO_BYTES = 512 * 1024**2
RAM_WORKSPACE = MAXIMUM_DECODED_AUDIO_BYTES * 4


def request(model_id: str, model_directory: Path) -> ResourceProfile:
    if model_id == "none":
        return ResourceProfile(active_ram=RAM_WORKSPACE)
    path = model_directory / model_id
    if not path.suffix:
        safetensors_path = path.with_suffix(".safetensors")
        path = (
            safetensors_path
            if safetensors_path.is_file()
            else path.with_suffix(".pth")
        )
    if not path.is_file():
        return ResourceProfile(
            resident_vram=RUNTIME_WEIGHTS,
            active_ram=RAM_WORKSPACE,
            active_vram=INFERENCE_WORKSPACE,
        )
    weight_bytes = (
        _safetensors_half_bytes(path)
        if path.suffix == ".safetensors"
        else path.stat().st_size
    )
    return ResourceProfile(
        resident_vram=weight_bytes + RUNTIME_WEIGHTS,
        active_ram=RAM_WORKSPACE,
        active_vram=INFERENCE_WORKSPACE,
    )


def _safetensors_half_bytes(path: Path) -> int:
    dtype_bytes = {
        "BOOL": 1,
        "U8": 1,
        "I8": 1,
        "I16": 2,
        "U16": 2,
        "I32": 4,
        "U32": 4,
        "I64": 8,
        "U64": 8,
    }
    try:
        with safe_open(path, framework="pt", device="cpu") as checkpoint:
            total = 0
            for name in checkpoint.keys():
                tensor = checkpoint.get_slice(name)
                dtype = str(tensor.get_dtype())
                total += math.prod(tensor.get_shape()) * dtype_bytes.get(dtype, 2)
            return total
    except (OSError, SafetensorError) as exception:
        raise ValueError(
            f"Could not inspect Pneuma model {path}: {exception}"
        ) from exception
