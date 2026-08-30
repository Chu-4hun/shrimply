import math

import torch
from torch import Tensor, nn
from torch.nn import functional as F


class WeightNormConv1d(nn.Module):
    def __init__(
        self,
        input_channels: int,
        output_channels: int,
        kernel_size: int,
        dilation: int = 1,
    ) -> None:
        super().__init__()
        weight = torch.empty(output_channels, input_channels, kernel_size)
        nn.init.kaiming_uniform_(weight, a=math.sqrt(5))
        self.weight_v = nn.Parameter(weight)
        self.weight_g = nn.Parameter(
            torch.linalg.vector_norm(weight, dim=(1, 2), keepdim=True)
        )
        self.bias = nn.Parameter(torch.empty(output_channels))
        bound = 1 / math.sqrt(input_channels * kernel_size)
        nn.init.uniform_(self.bias, -bound, bound)
        self._dilation = dilation
        self.weight_precomputed = False

    def forward(self, inputs: Tensor) -> Tensor:
        if self.weight_precomputed:
            weight = self.weight_v
        else:
            norm = torch.linalg.vector_norm(self.weight_v, dim=(1, 2), keepdim=True)
            weight = self.weight_v * self.weight_g / norm
        return F.conv1d(inputs, weight, self.bias, dilation=self._dilation)


class NormConv1d(nn.Module):
    def __init__(
        self,
        input_channels: int,
        output_channels: int,
        kernel_size: int,
        dilation: int,
    ) -> None:
        super().__init__()
        self.conv = WeightNormConv1d(
            input_channels, output_channels, kernel_size, dilation
        )
        self.norm = nn.Identity()

    def forward(self, inputs: Tensor) -> Tensor:
        return self.norm(self.conv(inputs))


class SConv1d(nn.Module):
    def __init__(
        self,
        input_channels: int,
        output_channels: int,
        kernel_size: int,
        dilation: int = 1,
    ) -> None:
        super().__init__()
        self.conv = NormConv1d(
            input_channels, output_channels, kernel_size, dilation
        )
        self._effective_kernel_size = (kernel_size - 1) * dilation + 1

    def forward(self, inputs: Tensor) -> Tensor:
        padding = self._effective_kernel_size - 1
        right = padding // 2
        left = padding - right
        return self.conv(F.pad(inputs, (left, right), mode="reflect"))


class WN(nn.Module):
    def __init__(
        self,
        hidden_channels: int,
        kernel_size: int,
        dilation_rate: int,
        layers: int,
        condition_channels: int,
        dropout: float,
    ) -> None:
        super().__init__()
        if kernel_size % 2 != 1:
            raise ValueError("WaveNet requires an odd kernel size")
        self.hidden_channels = hidden_channels
        self.n_layers = layers
        self.gin_channels = condition_channels
        self.in_layers = nn.ModuleList(
            SConv1d(
                hidden_channels,
                2 * hidden_channels,
                kernel_size,
                dilation=dilation_rate**index,
            )
            for index in range(layers)
        )
        self.res_skip_layers = nn.ModuleList(
            SConv1d(
                hidden_channels,
                2 * hidden_channels if index < layers - 1 else hidden_channels,
                1,
            )
            for index in range(layers)
        )
        self.drop = nn.Dropout(dropout)
        self.cond_layer = SConv1d(
            condition_channels,
            2 * hidden_channels * layers,
            1,
        )

    def forward(
        self,
        values: Tensor,
        mask: Tensor,
        condition: Tensor,
    ) -> Tensor:
        output = torch.zeros_like(values)
        projected_condition = self.cond_layer(condition)
        for index in range(self.n_layers):
            activations = self.in_layers[index](values)
            offset = index * 2 * self.hidden_channels
            layer_condition = projected_condition[
                :, offset : offset + 2 * self.hidden_channels, :
            ]
            tanh_values, sigmoid_values = (activations + layer_condition).split(
                self.hidden_channels, dim=1
            )
            gated = self.drop(torch.tanh(tanh_values) * torch.sigmoid(sigmoid_values))
            residual_and_skip = self.res_skip_layers[index](gated)
            if index < self.n_layers - 1:
                values = (
                    values + residual_and_skip[:, : self.hidden_channels, :]
                ) * mask
                output = output + residual_and_skip[:, self.hidden_channels :, :]
            else:
                output = output + residual_and_skip
        return output * mask
