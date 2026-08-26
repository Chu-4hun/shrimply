from typing import Literal

PitchMethod = Literal["crepe", "rmvpe", "fcpe", "swift-f0"]
DEFAULT_PITCH_METHOD: PitchMethod = "rmvpe"
PITCH_METHOD_CHOICES: tuple[PitchMethod, ...] = (
    "crepe",
    "rmvpe",
    "fcpe",
    "swift-f0",
)
ALL_PITCH_METHODS = frozenset(PITCH_METHOD_CHOICES)
