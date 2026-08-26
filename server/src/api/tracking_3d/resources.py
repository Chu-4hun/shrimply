from api.resource import ResourceProfile
from api.tracking_3d.protocol import ModelId

GIB = 1024**3
PROFILES = {
    # COLMAP has no persistent neural weights.
    "colmap/colmap": ResourceProfile(active_vram=2 * GIB),
    # VGGT's published 1.256B parameters are loaded at 2 bytes each.
    "MIT-SPARK/VGGT-SLAM": ResourceProfile(
        resident_vram=1_256_537_516 * 2,
        active_vram=4 * GIB,
    ),
}


def profile(model_id: ModelId) -> ResourceProfile:
    return PROFILES[model_id]
