from collections.abc import Sequence
from typing import Literal

from api.pneuma.inference.configs.v3_config import get_v3_config
from api.pneuma.inference.models.bigvgan import SpeakerConditionedBigVGANNSFGenerator
from api.pneuma.inference.models.models import SpeakerConditionedSynthesizerBase


class SynthesizerTrnBigVGANsid(SpeakerConditionedSynthesizerBase):
    ssl_dim = 1024

    def __init__(self, *, is_half: bool = False) -> None:
        super().__init__()
        self._init_from_config(
            get_v3_config(),
            sr=48000,
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
    ) -> SpeakerConditionedBigVGANNSFGenerator:
        return SpeakerConditionedBigVGANNSFGenerator(
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
def build_v3_synthesizer(is_half: bool) -> SynthesizerTrnBigVGANsid:
    return SynthesizerTrnBigVGANsid(is_half=is_half)
