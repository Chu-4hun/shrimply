import math
from dataclasses import dataclass

import torch
from torch import Tensor, nn
from torch.nn import functional as F

from .acoustic_transformer import Transformer, TransformerConfig
from .acoustic_wavenet import WN


@dataclass(frozen=True, slots=True)
class DiffusionTransformerConfig:
    mel_channels: int = 80
    hidden_dimension: int = 512
    heads: int = 8
    layers: int = 13
    content_codebook_size: int = 1_024
    content_dimension: int = 512
    style_dimension: int = 192
    sequence_length: int = 16_384
    class_dropout_probability: float = 0.1
    wavenet_hidden_dimension: int = 512
    wavenet_kernel_size: int = 5
    wavenet_dilation_rate: int = 1
    wavenet_layers: int = 8
    wavenet_dropout: float = 0.2


class WeightNormLinear(nn.Module):
    def __init__(self, input_dimension: int, output_dimension: int) -> None:
        super().__init__()
        weight = torch.empty(output_dimension, input_dimension)
        nn.init.kaiming_uniform_(weight, a=math.sqrt(5))
        self.weight_v = nn.Parameter(weight)
        self.weight_g = nn.Parameter(
            torch.linalg.vector_norm(weight, dim=1, keepdim=True)
        )
        self.bias = nn.Parameter(torch.empty(output_dimension))
        bound = 1 / math.sqrt(input_dimension)
        nn.init.uniform_(self.bias, -bound, bound)
        self.weight_precomputed = False

    def forward(self, inputs: Tensor) -> Tensor:
        if self.weight_precomputed:
            weight = self.weight_v
        else:
            norm = torch.linalg.vector_norm(self.weight_v, dim=1, keepdim=True)
            weight = self.weight_v * self.weight_g / norm
        return F.linear(inputs, weight, self.bias)


class TimestepEmbedder(nn.Module):
    freqs: Tensor

    def __init__(
        self,
        hidden_dimension: int,
        frequency_embedding_size: int = 256,
    ) -> None:
        super().__init__()
        self.mlp = nn.Sequential(
            nn.Linear(frequency_embedding_size, hidden_dimension),
            nn.SiLU(),
            nn.Linear(hidden_dimension, hidden_dimension),
        )
        self.frequency_embedding_size = frequency_embedding_size
        self.max_period = 10_000
        self.scale = 1_000
        half = frequency_embedding_size // 2
        frequencies = torch.exp(
            -math.log(self.max_period)
            * torch.arange(half, dtype=torch.float32)
            / half
        )
        self.register_buffer("freqs", frequencies)

    def forward(self, timesteps: Tensor) -> Tensor:
        arguments = (
            self.scale
            * timesteps[:, None].float()
            * self.freqs[None]
        )
        embedding = torch.cat((torch.cos(arguments), torch.sin(arguments)), dim=-1)
        if self.frequency_embedding_size % 2:
            embedding = torch.cat(
                (embedding, torch.zeros_like(embedding[:, :1])), dim=-1
            )
        return self.mlp(embedding.to(dtype=self.freqs.dtype))


class FinalLayer(nn.Module):
    def __init__(self, hidden_dimension: int) -> None:
        super().__init__()
        self.norm_final = nn.LayerNorm(
            hidden_dimension, elementwise_affine=False, eps=1e-6
        )
        self.linear = WeightNormLinear(hidden_dimension, hidden_dimension)
        self.adaLN_modulation = nn.Sequential(
            nn.SiLU(),
            nn.Linear(hidden_dimension, 2 * hidden_dimension),
        )

    def forward(self, values: Tensor, condition: Tensor) -> Tensor:
        shift, scale = self.adaLN_modulation(condition).chunk(2, dim=1)
        normalized = self.norm_final(values)
        modulated = normalized * (1 + scale.unsqueeze(1)) + shift.unsqueeze(1)
        return self.linear(modulated)


class DiffusionTransformer(nn.Module):
    input_pos: Tensor

    def __init__(self, config: DiffusionTransformerConfig) -> None:
        super().__init__()
        self.in_channels = config.mel_channels
        self.out_channels = config.mel_channels
        self.num_heads = config.heads
        transformer_config = TransformerConfig(
            sequence_length=config.sequence_length,
            layers=config.layers,
            heads=config.heads,
            dimension=config.hidden_dimension,
            head_dimension=config.hidden_dimension // config.heads,
            skip_connections=True,
        )
        self.transformer = Transformer(transformer_config)
        self.x_embedder = WeightNormLinear(
            config.mel_channels, config.hidden_dimension
        )
        self.content_type = "discrete"
        self.content_codebook_size = config.content_codebook_size
        self.content_dim = config.content_dimension
        self.cond_embedder = nn.Embedding(
            config.content_codebook_size, config.hidden_dimension
        )
        self.cond_projection = nn.Linear(
            config.content_dimension, config.hidden_dimension
        )
        self.t_embedder = TimestepEmbedder(config.hidden_dimension)
        self.register_buffer("input_pos", torch.arange(config.sequence_length))
        self.t_embedder2 = TimestepEmbedder(config.wavenet_hidden_dimension)
        self.conv1 = nn.Linear(
            config.hidden_dimension, config.wavenet_hidden_dimension
        )
        self.conv2 = nn.Conv1d(
            config.wavenet_hidden_dimension, config.mel_channels, 1
        )
        self.wavenet = WN(
            hidden_channels=config.wavenet_hidden_dimension,
            kernel_size=config.wavenet_kernel_size,
            dilation_rate=config.wavenet_dilation_rate,
            layers=config.wavenet_layers,
            condition_channels=config.wavenet_hidden_dimension,
            dropout=config.wavenet_dropout,
        )
        self.final_layer = FinalLayer(config.wavenet_hidden_dimension)
        self.res_projection = nn.Linear(
            config.hidden_dimension, config.wavenet_hidden_dimension
        )
        self.content_mask_embedder = nn.Embedding(1, config.hidden_dimension)
        self.skip_linear = nn.Linear(
            config.hidden_dimension + config.mel_channels,
            config.hidden_dimension,
        )
        merged_dimensions = (
            config.hidden_dimension
            + 2 * config.mel_channels
            + config.style_dimension
        )
        self.cond_x_merge_linear = nn.Linear(
            merged_dimensions, config.hidden_dimension
        )

    def prepare(self) -> None:
        parameter = self.transformer.norm.project_layer.weight
        self.transformer.prepare(parameter.device, parameter.dtype)

    def forward(
        self,
        noise: Tensor,
        prompt: Tensor,
        lengths: Tensor,
        timestep: Tensor,
        style: Tensor,
        condition: Tensor,
    ) -> Tensor:
        batch, _, length = noise.shape
        timestep_condition = self.t_embedder(timestep)
        projected_condition = self.cond_projection(condition)
        transposed_noise = noise.transpose(1, 2)
        transposed_prompt = prompt.transpose(1, 2)
        merged = torch.cat(
            (
                transposed_noise,
                transposed_prompt,
                projected_condition,
                style[:, None, :].expand(batch, length, -1),
            ),
            dim=-1,
        )
        transformer_input = self.cond_x_merge_linear(merged)
        mask = _sequence_mask(lengths, transformer_input.size(1)).to(noise.device)
        wavenet_mask = mask.unsqueeze(1)
        attention_mask = wavenet_mask[:, None, :].expand(
            -1, 1, transformer_input.size(1), -1
        )
        positions = self.input_pos[: transformer_input.size(1)]
        transformed = self.transformer(
            transformer_input,
            timestep_condition.unsqueeze(1),
            positions,
            attention_mask,
        )
        transformed = self.skip_linear(
            torch.cat((transformed, transposed_noise), dim=-1)
        )
        wavenet_input = self.conv1(transformed).transpose(1, 2)
        wavenet_condition = self.t_embedder2(timestep).unsqueeze(2)
        generated = self.wavenet(
            wavenet_input, wavenet_mask, wavenet_condition
        ).transpose(1, 2)
        generated = generated + self.res_projection(transformed)
        generated = self.final_layer(generated, timestep_condition).transpose(1, 2)
        return self.conv2(generated)


def _sequence_mask(lengths: Tensor, maximum_length: int) -> Tensor:
    positions = torch.arange(
        maximum_length, dtype=lengths.dtype, device=lengths.device
    )
    return positions.unsqueeze(0) < lengths.unsqueeze(1)
