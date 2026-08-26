import math

import torch
from torch import nn
from torch.nn import functional as functional


def make_pad_mask(lengths: torch.Tensor, maximum_length: int) -> torch.Tensor:
    positions = torch.arange(maximum_length, device=lengths.device)
    return positions.unsqueeze(0) >= lengths.unsqueeze(1)


class RelPositionalEncoding(nn.Module):
    def __init__(
        self, dimension: int, dropout_rate: float, maximum_length: int = 5000
    ) -> None:
        super().__init__()
        self.d_model = dimension
        self.xscale = math.sqrt(dimension)
        self.dropout = nn.Dropout(dropout_rate)
        self.max_len = maximum_length
        encoding = torch.zeros(maximum_length, dimension)
        position = torch.arange(maximum_length).unsqueeze(1)
        divisor = torch.exp(
            torch.arange(0, dimension, 2) * -(math.log(10_000.0) / dimension)
        )
        encoding[:, 0::2] = torch.sin(position * divisor)
        encoding[:, 1::2] = torch.cos(position * divisor)
        self.register_buffer("pe", encoding.unsqueeze(0))

    def forward(self, inputs: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
        if inputs.shape[1] >= self.max_len:
            raise ValueError("Conformer input exceeds its positional encoding")
        position_buffer = self._buffers["pe"]
        if position_buffer is None:
            raise RuntimeError("Conformer positional encoding is unavailable")
        position = position_buffer[:, : inputs.shape[1]].to(inputs.device)
        return self.dropout(inputs * self.xscale), self.dropout(position)


class Conv2dSubsampling2(nn.Module):
    def __init__(
        self,
        input_dimension: int,
        output_dimension: int,
        dropout_rate: float,
    ) -> None:
        super().__init__()
        self.conv = nn.Sequential(
            nn.Conv2d(1, output_dimension, 3, 2),
            nn.ReLU(),
        )
        self.out = nn.Sequential(
            nn.Linear(output_dimension * ((input_dimension - 1) // 2), output_dimension)
        )
        self.pos_enc = RelPositionalEncoding(output_dimension, dropout_rate)
        self.subsampling_rate = 2
        self.right_context = 2

    def forward(
        self, inputs: torch.Tensor, mask: torch.Tensor
    ) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
        convolved = self.conv(inputs.unsqueeze(1))
        batch, channels, time, features = convolved.shape
        flattened = convolved.transpose(1, 2).contiguous().view(
            batch, time, channels * features
        )
        encoded, position = self.pos_enc(self.out(flattened))
        return encoded, position, mask[:, :, 2::2]


class RelPositionMultiHeadedAttention(nn.Module):
    def __init__(self, heads: int, features: int, dropout_rate: float) -> None:
        super().__init__()
        if features % heads:
            raise ValueError("Conformer features must be divisible by attention heads")
        self.d_k = features // heads
        self.h = heads
        self.linear_q = nn.Linear(features, features)
        self.linear_k = nn.Linear(features, features)
        self.linear_v = nn.Linear(features, features)
        self.linear_out = nn.Linear(features, features)
        self.dropout = nn.Dropout(dropout_rate)
        self.linear_pos = nn.Linear(features, features, bias=False)
        self.pos_bias_u = nn.Parameter(torch.empty(heads, self.d_k))
        self.pos_bias_v = nn.Parameter(torch.empty(heads, self.d_k))
        nn.init.xavier_uniform_(self.pos_bias_u)
        nn.init.xavier_uniform_(self.pos_bias_v)

    def project(self, inputs: torch.Tensor, layer: nn.Linear) -> torch.Tensor:
        batch = inputs.shape[0]
        return layer(inputs).view(batch, -1, self.h, self.d_k).transpose(1, 2)

    def forward(
        self,
        query: torch.Tensor,
        key: torch.Tensor,
        value: torch.Tensor,
        mask: torch.Tensor,
        position: torch.Tensor,
    ) -> torch.Tensor:
        projected_query = self.project(query, self.linear_q).transpose(1, 2)
        projected_key = self.project(key, self.linear_k)
        projected_value = self.project(value, self.linear_v)
        projected_position = self.project(position, self.linear_pos)
        content_query = (projected_query + self.pos_bias_u).transpose(1, 2)
        position_query = (projected_query + self.pos_bias_v).transpose(1, 2)
        scores = torch.matmul(content_query, projected_key.transpose(-2, -1))
        scores += torch.matmul(position_query, projected_position.transpose(-2, -1))
        scores /= math.sqrt(self.d_k)
        invalid = ~mask.unsqueeze(1)[:, :, :, : scores.shape[-1]]
        weights = torch.softmax(scores.masked_fill(invalid, -torch.inf), dim=-1)
        weights = self.dropout(weights.masked_fill(invalid, 0.0))
        attended = torch.matmul(weights, projected_value)
        batch = attended.shape[0]
        merged = attended.transpose(1, 2).contiguous().view(
            batch, -1, self.h * self.d_k
        )
        return self.linear_out(merged)


class PositionwiseFeedForward(nn.Module):
    def __init__(
        self, features: int, hidden_units: int, dropout_rate: float
    ) -> None:
        super().__init__()
        self.w_1 = nn.Linear(features, hidden_units)
        self.activation = nn.SiLU()
        self.dropout = nn.Dropout(dropout_rate)
        self.w_2 = nn.Linear(hidden_units, features)

    def forward(self, inputs: torch.Tensor) -> torch.Tensor:
        return self.w_2(self.dropout(self.activation(self.w_1(inputs))))


class ConvolutionModule(nn.Module):
    def __init__(self, channels: int, kernel_size: int = 15) -> None:
        super().__init__()
        if (kernel_size - 1) % 2:
            raise ValueError("Conformer convolution kernel must be odd")
        self.pointwise_conv1 = nn.Conv1d(channels, 2 * channels, 1)
        self.lorder = 0
        self.depthwise_conv = nn.Conv1d(
            channels,
            channels,
            kernel_size,
            padding=(kernel_size - 1) // 2,
            groups=channels,
        )
        self.use_layer_norm = True
        self.norm = nn.LayerNorm(channels)
        self.pointwise_conv2 = nn.Conv1d(channels, channels, 1)
        self.activation = nn.SiLU()

    def forward(self, inputs: torch.Tensor, mask: torch.Tensor) -> torch.Tensor:
        convolved = inputs.transpose(1, 2)
        convolved.masked_fill_(~mask, 0.0)
        convolved = functional.glu(self.pointwise_conv1(convolved), dim=1)
        convolved = self.depthwise_conv(convolved).transpose(1, 2)
        convolved = self.activation(self.norm(convolved)).transpose(1, 2)
        convolved = self.pointwise_conv2(convolved)
        convolved.masked_fill_(~mask, 0.0)
        return convolved.transpose(1, 2)


class ConformerEncoderLayer(nn.Module):
    def __init__(
        self,
        size: int,
        attention: RelPositionMultiHeadedAttention,
        feed_forward: PositionwiseFeedForward,
        convolution: ConvolutionModule,
        dropout_rate: float,
    ) -> None:
        super().__init__()
        self.self_attn = attention
        self.feed_forward = feed_forward
        self.feed_forward_macaron = None
        self.conv_module = convolution
        self.norm_ff = nn.LayerNorm(size, eps=1e-5)
        self.norm_mha = nn.LayerNorm(size, eps=1e-5)
        self.ff_scale = 1.0
        self.norm_conv = nn.LayerNorm(size, eps=1e-5)
        self.norm_final = nn.LayerNorm(size, eps=1e-5)
        self.dropout = nn.Dropout(dropout_rate)
        self.size = size
        self.normalize_before = True
        self.concat_after = False
        self.concat_linear = nn.Identity()

    def forward(
        self,
        inputs: torch.Tensor,
        mask: torch.Tensor,
        position: torch.Tensor,
    ) -> tuple[torch.Tensor, torch.Tensor]:
        residual = inputs
        normalized = self.norm_mha(inputs)
        attended = self.self_attn(
            normalized,
            normalized,
            normalized,
            mask,
            position,
        )
        encoded = residual + self.dropout(attended)
        residual = encoded
        encoded = residual + self.dropout(
            self.conv_module(self.norm_conv(encoded), mask)
        )
        residual = encoded
        encoded = residual + self.ff_scale * self.dropout(
            self.feed_forward(self.norm_ff(encoded))
        )
        return self.norm_final(encoded), mask


class ConformerEncoder(nn.Module):
    def __init__(
        self,
        input_size: int,
        output_size: int,
        attention_heads: int,
        linear_units: int,
        num_blocks: int,
        input_layer: str,
    ) -> None:
        super().__init__()
        if input_layer != "conv2d2":
            raise ValueError("IndexTTS 2 requires the conv2d2 Conformer input")
        self._output_size = output_size
        self.embed = Conv2dSubsampling2(input_size, output_size, 0.0)
        self.normalize_before = True
        self.after_norm = nn.LayerNorm(output_size, eps=1e-5)
        self.encoders = nn.ModuleList(
            ConformerEncoderLayer(
                output_size,
                RelPositionMultiHeadedAttention(
                    attention_heads, output_size, 0.0
                ),
                PositionwiseFeedForward(output_size, linear_units, 0.0),
                ConvolutionModule(output_size),
                0.0,
            )
            for _ in range(num_blocks)
        )

    def forward(
        self, inputs: torch.Tensor, lengths: torch.Tensor
    ) -> tuple[torch.Tensor, torch.Tensor]:
        mask = ~make_pad_mask(lengths, inputs.shape[1]).unsqueeze(1)
        encoded, position, mask = self.embed(inputs, mask)
        for layer in self.encoders:
            encoded, mask = layer(encoded, mask, position)
        return self.after_norm(encoded), mask
