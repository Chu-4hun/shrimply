from __future__ import annotations

from typing import Final, Literal

import numpy as np

TRAINING_SAMPLE_RATE: Final[Literal[48_000]] = 48_000
TRAINING_SAMPLE_RATE_NAME: Final[Literal["48k"]] = "48k"

type MonoAudioArray = np.ndarray
type ResampledMonoAudioArray = np.ndarray
type StereoAudioArray = np.ndarray
type RawMonoAudioArray = np.ndarray
type RawStereoAudioArray = np.ndarray
type RawAudioArray = RawMonoAudioArray | RawStereoAudioArray
type OutputAudioArray = np.ndarray
type AudioInput = tuple[int, RawAudioArray]

__all__ = [
    "AudioInput",
    "MonoAudioArray",
    "OutputAudioArray",
    "RawAudioArray",
    "RawMonoAudioArray",
    "RawStereoAudioArray",
    "ResampledMonoAudioArray",
    "StereoAudioArray",
    "TRAINING_SAMPLE_RATE",
    "TRAINING_SAMPLE_RATE_NAME",
]

