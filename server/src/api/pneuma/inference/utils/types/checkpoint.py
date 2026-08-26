from typing import Literal, TypedDict

import torch

from .version import ModelVersion


type IntSequence = list[int] | tuple[int, ...]
type NestedIntSequence = list[IntSequence] | tuple[IntSequence, ...]
type SynthesizerConfigValue = int | float | str | IntSequence | NestedIntSequence | None
type SynthesizerConfig = list[SynthesizerConfigValue]
type WeightMap = dict[str, torch.Tensor]


class VoiceCheckpoint(TypedDict):
    weight: WeightMap
    sample_rate: Literal[48000]
    version: ModelVersion
    epoch: int
