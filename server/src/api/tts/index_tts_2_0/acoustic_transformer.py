from dataclasses import dataclass

import torch
from torch import Tensor, nn
from torch.nn import functional as F


def _next_multiple(value: int, divisor: int) -> int:
    remainder = value % divisor
    return value if remainder == 0 else value + divisor - remainder


@dataclass(frozen=True, slots=True)
class TransformerConfig:
    sequence_length: int
    layers: int
    heads: int
    dimension: int
    head_dimension: int
    skip_connections: bool
    rope_base: int = 10_000
    norm_epsilon: float = 1e-5
    intermediate_size: int = 0

    def feed_forward_size(self) -> int:
        if self.intermediate_size:
            return self.intermediate_size
        return _next_multiple(int(8 * self.dimension / 3), 256)


class AdaptiveLayerNorm(nn.Module):
    def __init__(self, dimension: int, epsilon: float) -> None:
        super().__init__()
        self.project_layer = nn.Linear(dimension, 2 * dimension)
        self.norm = RMSNorm(dimension, epsilon)
        self.d_model = dimension
        self.use_triton = False

    def forward(self, values: Tensor, condition: Tensor) -> Tensor:
        weight, bias = self.project_layer(condition).split(self.d_model, dim=-1)
        if self.use_triton:
            from api.tts.index_triton import adaptive_rms_norm

            return adaptive_rms_norm(
                values,
                weight,
                bias,
                self.norm.weight,
                self.norm.eps,
            )
        return weight * self.norm(values) + bias


class RMSNorm(nn.Module):
    def __init__(self, dimension: int, epsilon: float) -> None:
        super().__init__()
        self.eps = epsilon
        self.weight = nn.Parameter(torch.ones(dimension))

    def forward(self, values: Tensor) -> Tensor:
        normalized = values.float() * torch.rsqrt(
            torch.mean(values.float().square(), dim=-1, keepdim=True) + self.eps
        )
        return normalized.to(values.dtype) * self.weight


class Attention(nn.Module):
    def __init__(self, config: TransformerConfig) -> None:
        super().__init__()
        if config.dimension != config.heads * config.head_dimension:
            raise ValueError("The transformer dimension must equal heads times head dimension")
        projected_dimension = 3 * config.heads * config.head_dimension
        self.wqkv = nn.Linear(config.dimension, projected_dimension, bias=False)
        self.wo = nn.Linear(config.dimension, config.dimension, bias=False)
        self.n_head = config.heads
        self.head_dim = config.head_dimension
        self.n_local_heads = config.heads

    def forward(self, values: Tensor, rotations: Tensor, mask: Tensor) -> Tensor:
        batch, length, _ = values.shape
        query, key, value = self.wqkv(values).chunk(3, dim=-1)
        query = query.view(batch, length, self.n_head, self.head_dim)
        key = key.view(batch, length, self.n_local_heads, self.head_dim)
        value = value.view(batch, length, self.n_local_heads, self.head_dim)
        query = _apply_rotary_embedding(query, rotations).transpose(1, 2)
        key = _apply_rotary_embedding(key, rotations).transpose(1, 2)
        value = value.transpose(1, 2)
        attended = F.scaled_dot_product_attention(
            query,
            key,
            value,
            attn_mask=mask,
            dropout_p=0.0,
        )
        attended = attended.transpose(1, 2).contiguous().view(batch, length, -1)
        return self.wo(attended)


class FeedForward(nn.Module):
    def __init__(self, config: TransformerConfig) -> None:
        super().__init__()
        hidden = config.feed_forward_size()
        self.w1 = nn.Linear(config.dimension, hidden, bias=False)
        self.w3 = nn.Linear(config.dimension, hidden, bias=False)
        self.w2 = nn.Linear(hidden, config.dimension, bias=False)

    def forward(self, values: Tensor) -> Tensor:
        return self.w2(F.silu(self.w1(values)) * self.w3(values))


class TransformerBlock(nn.Module):
    def __init__(self, config: TransformerConfig) -> None:
        super().__init__()
        self.attention = Attention(config)
        self.feed_forward = FeedForward(config)
        self.ffn_norm = AdaptiveLayerNorm(config.dimension, config.norm_epsilon)
        self.attention_norm = AdaptiveLayerNorm(config.dimension, config.norm_epsilon)
        self.uvit_skip_connection = config.skip_connections
        if config.skip_connections:
            self.skip_in_linear = nn.Linear(2 * config.dimension, config.dimension)

    def forward(
        self,
        values: Tensor,
        condition: Tensor,
        rotations: Tensor,
        mask: Tensor,
        skip: Tensor | None,
    ) -> Tensor:
        if self.uvit_skip_connection and skip is not None:
            values = self.skip_in_linear(torch.cat((values, skip), dim=-1))
        residual = values + self.attention(
            self.attention_norm(values, condition), rotations, mask
        )
        return residual + self.feed_forward(self.ffn_norm(residual, condition))


class Transformer(nn.Module):
    def __init__(self, config: TransformerConfig) -> None:
        super().__init__()
        self.layers = nn.ModuleList(TransformerBlock(config) for _ in range(config.layers))
        self.norm = AdaptiveLayerNorm(config.dimension, config.norm_epsilon)
        self._config = config
        self._rotations: Tensor | None = None

    def prepare(self, device: torch.device, dtype: torch.dtype) -> None:
        self._rotations = _precompute_rotations(
            self._config.sequence_length,
            self._config.head_dimension,
            self._config.rope_base,
            dtype,
        ).to(device)

    def forward(
        self,
        values: Tensor,
        condition: Tensor,
        positions: Tensor,
        mask: Tensor,
    ) -> Tensor:
        if self._rotations is None:
            raise RuntimeError("The acoustic transformer must be prepared before inference")
        rotations = self._rotations[positions]
        emitted: list[Tensor] = []
        middle = len(self.layers) // 2
        for index, layer in enumerate(self.layers):
            skip = emitted.pop() if self._config.skip_connections and index > middle else None
            values = layer(values, condition, rotations, mask, skip)
            if self._config.skip_connections and index < middle:
                emitted.append(values)
        return self.norm(values, condition)


def _precompute_rotations(
    sequence_length: int,
    elements: int,
    base: int,
    dtype: torch.dtype,
) -> Tensor:
    frequencies = 1.0 / (
        base ** (torch.arange(0, elements, 2, dtype=torch.float32) / elements)
    )
    phases = torch.outer(torch.arange(sequence_length), frequencies)
    complex_frequencies = torch.polar(torch.ones_like(phases), phases)
    return torch.stack((complex_frequencies.real, complex_frequencies.imag), dim=-1).to(
        dtype
    )


def _apply_rotary_embedding(values: Tensor, rotations: Tensor) -> Tensor:
    paired = values.float().reshape(*values.shape[:-1], -1, 2)
    rotations = rotations.view(1, paired.size(1), 1, paired.size(3), 2)
    rotated = torch.stack(
        (
            paired[..., 0] * rotations[..., 0]
            - paired[..., 1] * rotations[..., 1],
            paired[..., 1] * rotations[..., 0]
            + paired[..., 0] * rotations[..., 1],
        ),
        dim=-1,
    )
    return rotated.flatten(3).to(values.dtype)
