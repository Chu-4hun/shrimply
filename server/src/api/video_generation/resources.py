from api.resource import ResourceProfile
from api.video_generation.minimax_h3.inference import GenerationRequest as H3Request
from api.video_generation.wan.inference import (
    WAN21_MODEL_ID,
    WAN22_MODEL_ID,
    GenerationRequest as WanRequest,
)

GIB = 1024**3


def profile(prepared: H3Request | WanRequest) -> ResourceProfile:
    if isinstance(prepared, WanRequest):
        match prepared.model:
            case value if value == WAN21_MODEL_ID:
                return ResourceProfile(
                    resident_ram=20 * GIB,
                    resident_vram=3 * GIB,
                    active_ram=4 * GIB,
                    active_vram=7 * GIB,
                )
            case value if value == WAN22_MODEL_ID:
                return ResourceProfile(
                    resident_ram=24 * GIB,
                    resident_vram=10 * GIB,
                    active_ram=8 * GIB,
                    active_vram=14 * GIB,
                )
            case _:
                raise ValueError(f"Unknown Wan model: {prepared.model}")
    match prepared.memory:
        case "bf16":
            return ResourceProfile(
                resident_ram=120 * GIB,
                resident_vram=50 * GIB,
                active_ram=20 * GIB,
                active_vram=20 * GIB,
            )
        case "stream":
            return ResourceProfile(
                resident_ram=120 * GIB,
                resident_vram=18 * GIB,
                active_ram=20 * GIB,
                active_vram=6 * GIB,
            )
        case "int8":
            return ResourceProfile(
                resident_ram=64 * GIB,
                resident_vram=36 * GIB,
                active_ram=16 * GIB,
                active_vram=12 * GIB,
            )
        case "auto" | "hybrid":
            return ResourceProfile(resident_vram=50 * GIB, active_vram=20 * GIB)
        case _:
            raise ValueError(f"Unknown MiniMax H3 memory mode: {prepared.memory}")
