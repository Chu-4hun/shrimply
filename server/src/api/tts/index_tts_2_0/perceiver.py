import torch
from torch import nn
from torch.nn import functional as functional


class RMSNorm(nn.Module):
    def __init__(self, dimension: int) -> None:
        super().__init__()
        self.scale = dimension**0.5
        self.gamma = nn.Parameter(torch.ones(dimension))

    def forward(self, inputs: torch.Tensor) -> torch.Tensor:
        return functional.normalize(inputs, dim=-1) * self.scale * self.gamma


class GEGLU(nn.Module):
    def forward(self, inputs: torch.Tensor) -> torch.Tensor:
        values, gates = inputs.chunk(2, dim=-1)
        return functional.gelu(gates) * values


def feed_forward(dimension: int, multiplier: int = 4) -> nn.Sequential:
    inner_dimension = dimension * multiplier * 2 // 3
    return nn.Sequential(
        nn.Linear(dimension, inner_dimension * 2),
        GEGLU(),
        nn.Linear(inner_dimension, dimension),
    )


class Attend(nn.Module):
    def __init__(self, dropout: float = 0.0) -> None:
        super().__init__()
        self.attn_dropout = nn.Dropout(dropout)

    def forward(
        self,
        query: torch.Tensor,
        key: torch.Tensor,
        value: torch.Tensor,
        mask: torch.Tensor | None = None,
    ) -> torch.Tensor:
        scale = query.shape[-1] ** -0.5
        similarities = torch.einsum("bhid,bhjd->bhij", query, key) * scale
        if mask is not None:
            similarities = similarities.masked_fill(
                ~mask[:, None, None, :], -torch.finfo(similarities.dtype).max
            )
        weights = self.attn_dropout(similarities.softmax(dim=-1))
        return torch.einsum("bhij,bhjd->bhid", weights, value)


class Attention(nn.Module):
    def __init__(
        self,
        dimension: int,
        context_dimension: int | None = None,
        head_dimension: int = 64,
        heads: int = 8,
        dropout: float = 0.0,
        cross_attn_include_queries: bool = False,
    ) -> None:
        super().__init__()
        self.heads = heads
        self.head_dimension = head_dimension
        self.cross_attn_include_queries = cross_attn_include_queries
        inner_dimension = head_dimension * heads
        self.attend = Attend(dropout)
        self.to_q = nn.Linear(dimension, inner_dimension, bias=False)
        self.to_kv = nn.Linear(
            context_dimension or dimension, inner_dimension * 2, bias=False
        )
        self.to_out = nn.Linear(inner_dimension, dimension, bias=False)

    def split_heads(self, inputs: torch.Tensor) -> torch.Tensor:
        batch, length, _ = inputs.shape
        return inputs.reshape(
            batch, length, self.heads, self.head_dimension
        ).transpose(1, 2)

    def forward(
        self,
        inputs: torch.Tensor,
        context: torch.Tensor | None = None,
        mask: torch.Tensor | None = None,
    ) -> torch.Tensor:
        context_inputs = inputs if context is None else context
        if context is not None and self.cross_attn_include_queries:
            context_inputs = torch.cat((inputs, context_inputs), dim=-2)
        query = self.split_heads(self.to_q(inputs))
        key, value = self.to_kv(context_inputs).chunk(2, dim=-1)
        attended = self.attend(
            query,
            self.split_heads(key),
            self.split_heads(value),
            mask,
        )
        batch, _, length, _ = attended.shape
        merged = attended.transpose(1, 2).reshape(batch, length, -1)
        return self.to_out(merged)


class PerceiverResampler(nn.Module):
    def __init__(
        self,
        dimension: int,
        context_dimension: int | None = None,
        depth: int = 2,
        latent_count: int = 32,
        head_dimension: int = 64,
        heads: int = 8,
        feed_forward_multiplier: int = 4,
    ) -> None:
        super().__init__()
        resolved_context_dimension = context_dimension or dimension
        self.proj_context = (
            nn.Linear(resolved_context_dimension, dimension)
            if resolved_context_dimension != dimension
            else nn.Identity()
        )
        self.latents = nn.Parameter(torch.empty(latent_count, dimension))
        nn.init.normal_(self.latents, std=0.02)
        self.layers = nn.ModuleList(
            nn.ModuleList(
                [
                    Attention(
                        dimension,
                        context_dimension=dimension,
                        head_dimension=head_dimension,
                        heads=heads,
                        cross_attn_include_queries=True,
                    ),
                    feed_forward(dimension, feed_forward_multiplier),
                ]
            )
            for _ in range(depth)
        )
        self.norm = RMSNorm(dimension)

    def forward(
        self, context: torch.Tensor, mask: torch.Tensor | None = None
    ) -> torch.Tensor:
        projected_context = self.proj_context(context)
        latents = self.latents.unsqueeze(0).expand(context.shape[0], -1, -1)
        for layer in self.layers:
            if not isinstance(layer, nn.ModuleList):
                raise TypeError("Perceiver has an invalid layer")
            attention = layer[0]
            feed_forward_layer = layer[1]
            if not isinstance(attention, Attention) or not isinstance(
                feed_forward_layer, nn.Sequential
            ):
                raise TypeError("Perceiver layer has invalid components")
            latents = attention(latents, projected_context, mask) + latents
            latents = feed_forward_layer(latents) + latents
        return self.norm(latents)
