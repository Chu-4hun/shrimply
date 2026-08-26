from __future__ import annotations

from dataclasses import dataclass
from fractions import Fraction
from pathlib import Path
from urllib.parse import urlparse

MODEL_ID = "MiniMaxAI/MiniMax-H3"
FPS = 24
MIN_DURATION = Fraction(5)
MAX_DURATION = Fraction(15)
FRAME_CHUNK = 17
FRAME_REMAINDER = 5
CANVAS_MULTIPLE = 32


def align_num_frames(duration: Fraction | int) -> int:
    """Convert seconds to H3's next decodable frame count (17*n + 5)."""
    duration = Fraction(duration)
    if duration < MIN_DURATION or duration > MAX_DURATION:
        raise ValueError("duration must be between 5 and 15 seconds")
    scaled = duration * FPS
    frames = -(-scaled.numerator // scaled.denominator)
    while frames % FRAME_CHUNK != FRAME_REMAINDER:
        frames += 1
    if Fraction(frames, FPS) > MAX_DURATION:
        raise ValueError("duration aligns beyond MiniMax H3's 15-second maximum")
    return frames


def validate_canvas(width: int, height: int) -> None:
    if width <= 0 or height <= 0:
        raise ValueError("width and height must be positive")
    if width % CANVAS_MULTIPLE or height % CANVAS_MULTIPLE:
        raise ValueError(f"width and height must be multiples of {CANVAS_MULTIPLE}")
    ratio = width / height
    if not 0.25 <= ratio <= 4:
        raise ValueError("aspect ratio must be between 1:4 and 4:1")


def is_url(value: str) -> bool:
    return urlparse(value).scheme in {"http", "https"}


@dataclass(frozen=True)
class ReferenceSpec:
    kind: str
    source: str

    @classmethod
    def parse(cls, value: str) -> ReferenceSpec:
        try:
            kind, source = value.split(":", 1)
        except ValueError as exc:
            raise ValueError("reference must use TYPE:PATH, where TYPE is image, video, or audio") from exc
        if kind not in {"image", "video", "audio"}:
            raise ValueError(f"unsupported reference type {kind!r}; use image, video, or audio")
        if not source:
            raise ValueError("reference path cannot be empty")
        if not is_url(source) and not Path(source).is_file():
            raise ValueError(f"reference file does not exist: {source}")
        return cls(kind, source)


def validate_references(references: list[ReferenceSpec]) -> None:
    if not references:
        raise ValueError("ref2va requires at least one reference")
    if len(references) > 12:
        raise ValueError("H3 supports at most 12 total references")
    counts = {kind: sum(ref.kind == kind for ref in references) for kind in ("image", "video", "audio")}
    limits = {"image": 9, "video": 3, "audio": 3}
    for kind, limit in limits.items():
        if counts[kind] > limit:
            raise ValueError(f"H3 supports at most {limit} {kind} references")
    if counts["audio"] == len(references):
        raise ValueError("audio references cannot be the only reference modality")
