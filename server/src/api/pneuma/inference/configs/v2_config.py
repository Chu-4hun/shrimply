from dataclasses import dataclass, field
from typing import Literal


@dataclass(frozen=True, slots=True)
class V2DataConfig:
    sampling_rate: int = 48_000
    filter_length: int = 2_048


@dataclass(frozen=True, slots=True)
class V2ModelConfig:
    inter_channels: int = 192
    hidden_channels: int = 192
    filter_channels: int = 768
    n_heads: int = 2
    n_layers: int = 6
    kernel_size: int = 3
    p_dropout: int = 0
    resblock: Literal["1", "2"] = "1"
    resblock_kernel_sizes: tuple[int, ...] = (3, 7, 11)
    resblock_dilation_sizes: tuple[tuple[int, ...], ...] = (
        (1, 3, 5),
        (1, 3, 5),
        (1, 3, 5),
    )
    upsample_rates: tuple[int, ...] = (12, 10, 2, 2)
    upsample_initial_channel: int = 512
    upsample_kernel_sizes: tuple[int, ...] = (24, 20, 4, 4)
    gin_channels: int = 256
    spk_embed_dim: int = 109


@dataclass(frozen=True, slots=True)
class V2Config:
    data: V2DataConfig = field(default_factory=V2DataConfig)
    model: V2ModelConfig = field(default_factory=V2ModelConfig)


def get_v2_config() -> V2Config:
    return V2Config()
