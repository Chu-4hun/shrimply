import os
from pathlib import Path


def _positive_int(name: str, default: int) -> int:
    value = int(os.environ.get(name, default))
    if value <= 0:
        raise ValueError(f"{name} must be positive")
    return value


def _path(name: str, default: Path) -> Path:
    configured = os.environ.get(name)
    return Path(configured).expanduser().resolve() if configured else default


SERVER_GIT_HASH = os.environ.get("SHRIMPLY_SERVER_GIT_HASH", "").strip()
SERVER_HOST = os.environ.get("SHRIMPLY_SERVER_HOST", "0.0.0.0")
SERVER_PORT = _positive_int("SHRIMPLY_SERVER_PORT", 8787)
_server_share_value = os.environ.get("SHRIMPLY_SERVER_SHARE", "0")
if _server_share_value not in {"0", "1"}:
    raise ValueError("SHRIMPLY_SERVER_SHARE must be 0 or 1")
SERVER_SHARE = _server_share_value == "1"

MODEL_IDLE_TTL_SECONDS = _positive_int("SHRIMPLY_MODEL_IDLE_TTL_SECONDS", 10 * 60)
PNEUMA_MODEL_DIRECTORY = Path(os.environ.get("SHRIMPLY_PNEUMA_MODEL_DIR", "models"))


def video_generation_cache_root() -> Path:
    return _path(
        "SHRIMPLY_VIDEO_GENERATION_CACHE", Path.cwd() / "video-generation-cache"
    )


def minimax_h3_cache_directory() -> str | None:
    return os.environ.get("MINIMAX_H3_CACHE") or None


def minimax_h3_quantized_cache_root() -> Path:
    return _path("MINIMAX_H3_QUANTIZED_CACHE", Path.cwd() / "quantized-cache")


def minimax_h3_disk_offload_cache_root() -> Path:
    return _path("MINIMAX_H3_DISK_OFFLOAD_CACHE", Path.cwd() / "disk-offload-cache")


def minimax_h3_lora_cache_root() -> Path:
    return _path("MINIMAX_H3_LORA_CACHE", Path.cwd() / "lora-cache")


def minimax_h3_decode_offload() -> str:
    value = os.environ.get("MINIMAX_H3_DECODE_OFFLOAD", "auto").lower()
    if value not in {"auto", "gpu", "disk"}:
        raise ValueError("MINIMAX_H3_DECODE_OFFLOAD must be auto, gpu, or disk")
    return value


def set_pneuma_device(device: str) -> None:
    os.environ["SHRIMPLY_PNEUMA_DEVICE"] = device


def pneuma_device() -> str:
    return os.environ.get("SHRIMPLY_PNEUMA_DEVICE", "cpu")


def configure_video_generation_worker() -> None:
    os.environ.update(
        {
            "MALLOC_ARENA_MAX": "2",
            "OMP_NUM_THREADS": "1",
            "MKL_NUM_THREADS": "1",
            "TOKENIZERS_PARALLELISM": "false",
        }
    )
