import math
from typing import TypeVar

import torch
from torch import nn

Module = TypeVar("Module", bound=nn.Module)


def zero_parameters(module: Module) -> Module:
    for parameter in module.parameters():
        parameter.detach().zero_()
    return module


class GroupNorm32(nn.GroupNorm):
    def forward(self, input: torch.Tensor) -> torch.Tensor:
        return super().forward(input.float()).to(input.dtype)


def normalization(channels: int) -> GroupNorm32:
    groups = 8 if channels <= 16 else 16 if channels <= 64 else 32
    while channels % groups:
        groups //= 2
    if groups <= 2:
        raise ValueError(f"Cannot group-normalize {channels} channels")
    return GroupNorm32(groups, channels)


class QKVAttentionLegacy(nn.Module):
    def __init__(self, heads: int) -> None:
        super().__init__()
        self.n_heads = heads

    def forward(
        self, qkv: torch.Tensor, mask: torch.Tensor | None = None
    ) -> torch.Tensor:
        batch, width, length = qkv.shape
        if width % (3 * self.n_heads):
            raise ValueError("QKV channels must be divisible by three times the heads")
        channels = width // (3 * self.n_heads)
        query, key, value = qkv.reshape(
            batch * self.n_heads, channels * 3, length
        ).split(channels, dim=1)
        scale = 1 / math.sqrt(math.sqrt(channels))
        weights = torch.einsum("bct,bcs->bts", query * scale, key * scale)
        weights = torch.softmax(weights.float(), dim=-1).to(weights.dtype)
        if mask is not None:
            weights = weights * mask.repeat(self.n_heads, 1).unsqueeze(1)
        attended = torch.einsum("bts,bcs->bct", weights, value)
        return attended.reshape(batch, -1, length)


class AttentionBlock(nn.Module):
    def __init__(self, channels: int, heads: int = 1) -> None:
        super().__init__()
        self.channels = channels
        self.norm = normalization(channels)
        self.qkv = nn.Conv1d(channels, channels * 3, 1)
        self.attention = QKVAttentionLegacy(heads)
        self.proj_out = zero_parameters(nn.Conv1d(channels, channels, 1))

    def forward(
        self, inputs: torch.Tensor, mask: torch.Tensor | None = None
    ) -> torch.Tensor:
        batch, channels, *spatial = inputs.shape
        flattened = inputs.reshape(batch, channels, -1)
        qkv = self.qkv(self.norm(flattened))
        attended = self.proj_out(self.attention(qkv, mask))
        return (flattened + attended).reshape(batch, channels, *spatial)
