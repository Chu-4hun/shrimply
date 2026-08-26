from dataclasses import dataclass, field
from typing import Literal


@dataclass(frozen=True, slots=True)
class V33DataConfig:
    sampling_rate: int = 48_000
    filter_length: int = 2_048


@dataclass(frozen=True, slots=True)
class V33ModelConfig:
    inter_channels: int = 256
    hidden_channels: int = 256
    filter_channels: int = 1_024
    n_heads: int = 4
    n_layers: int = 6
    kernel_size: int = 3
    p_dropout: float = 0.0
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
    gin_channels: None = None
    spk_embed_dim: int = 0


@dataclass(frozen=True, slots=True)
class V33Config:
    data: V33DataConfig = field(default_factory=V33DataConfig)
    model: V33ModelConfig = field(default_factory=V33ModelConfig)


def get_v33_config() -> V33Config:
    return V33Config()
