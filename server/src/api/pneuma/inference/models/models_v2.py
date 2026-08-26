from collections.abc import Sequence
from typing import Literal

from api.pneuma.inference.configs.v2_config import get_v2_config
from api.pneuma.inference.models.generators import GeneratorNSF
from api.pneuma.inference.models.models import SpeakerConditionedSynthesizerBase
from api.pneuma.inference.utils.types import TRAINING_SAMPLE_RATE


class SynthesizerTrnNSFsid(SpeakerConditionedSynthesizerBase):
    def __init__(
        self,
        sr: str | int,
        *,
        is_half: bool = False,
    ) -> None:
        super().__init__()
        if isinstance(sr, str):
            if sr not in {"48k", "48000"}:
                raise ValueError("v2 synthesizer only supports 48k.")
            sr = TRAINING_SAMPLE_RATE
        if sr != TRAINING_SAMPLE_RATE:
            raise ValueError("v2 synthesizer only supports 48k.")
        self._init_from_config(
            get_v2_config(),
            sr=TRAINING_SAMPLE_RATE,
            is_half=is_half,
        )

    def _build_decoder(
        self,
        initial_channel: int,
        resblock: Literal["1", "2"],
        resblock_kernel_sizes: Sequence[int],
        resblock_dilation_sizes: Sequence[Sequence[int]],
        upsample_rates: Sequence[int],
        upsample_initial_channel: int,
        upsample_kernel_sizes: Sequence[int],
        gin_channels: int,
        sr: int,
        is_half: bool,
    ) -> GeneratorNSF:
        return GeneratorNSF(
            initial_channel,
            resblock,
            resblock_kernel_sizes,
            resblock_dilation_sizes,
            upsample_rates,
            upsample_initial_channel,
            upsample_kernel_sizes,
            gin_channels=gin_channels,
            sr=sr,
            is_half=is_half,
        )
def build_v2_synthesizer(is_half: bool) -> SynthesizerTrnNSFsid:
    return SynthesizerTrnNSFsid(TRAINING_SAMPLE_RATE, is_half=is_half)
