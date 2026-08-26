from collections.abc import Sequence

import torch
from torch import nn
from torch.nn import Conv1d
from torch.nn import functional as F
from torch.nn.utils.parametrizations import weight_norm

from api.pneuma.inference.models import utils
from api.pneuma.inference.models.utils import (
    # assert_float,
    get_padding,
    init_weights,
    remove_weight_norm,
)

LRELU_SLOPE = 0.1


class LayerNorm(nn.Module):
    def __init__(self, channels: int, eps: float = 1e-5) -> None:
        super(LayerNorm, self).__init__()
        self.channels = channels
        self.eps = eps

        self.gamma = nn.Parameter(torch.ones(channels))
        self.beta = nn.Parameter(torch.zeros(channels))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        x = x.transpose(1, -1)
        x = F.layer_norm(x, (self.channels,), self.gamma, self.beta, self.eps)
        return x.transpose(1, -1)


class RMSNorm(nn.Module):
    def __init__(self, channels: int, eps: float = 1e-6) -> None:
        super(RMSNorm, self).__init__()
        self.channels = channels
        self.eps = eps
        self.gamma = nn.Parameter(torch.ones(channels))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        x = x.transpose(1, -1)
        input_dtype = x.dtype
        x = x.to(torch.float32)
        variance = x.pow(2).mean(-1, keepdim=True)
        x = x * torch.rsqrt(variance + self.eps)
        x = self.gamma * x
        x = x.to(input_dtype)
        return x.transpose(1, -1)


class ConvReluNorm(nn.Module):
    def __init__(
        self,
        in_channels: int,
        hidden_channels: int,
        out_channels: int,
        kernel_size: int,
        n_layers: int,
        p_dropout: float,
    ) -> None:
        super(ConvReluNorm, self).__init__()
        self.in_channels = in_channels
        self.hidden_channels = hidden_channels
        self.out_channels = out_channels
        self.kernel_size = kernel_size
        self.n_layers = n_layers
        self.p_dropout = p_dropout
        assert n_layers > 1, "Number of layers should be larger than 0."

        self.conv_layers = nn.ModuleList()
        self.norm_layers = nn.ModuleList()
        self.conv_layers.append(
            nn.Conv1d(
                in_channels, hidden_channels, kernel_size, padding=kernel_size // 2
            )
        )
        self.norm_layers.append(LayerNorm(hidden_channels))
        self.relu_drop = nn.Sequential(nn.ReLU(), nn.Dropout(self.p_dropout))
        for _ in range(n_layers - 1):
            self.conv_layers.append(
                nn.Conv1d(
                    hidden_channels,
                    hidden_channels,
                    kernel_size,
                    padding=kernel_size // 2,
                )
            )
            self.norm_layers.append(LayerNorm(hidden_channels))
        self.proj = nn.Conv1d(hidden_channels, out_channels, 1)
        self.proj.weight.data.zero_()
        assert self.proj.bias is not None
        self.proj.bias.data.zero_()

    def forward(self, x: torch.Tensor, x_mask: torch.Tensor) -> torch.Tensor:
        x_org = x
        for i in range(self.n_layers):
            x = self.conv_layers[i](x * x_mask)
            x = self.norm_layers[i](x)
            x = self.relu_drop(x)
        x = x_org + self.proj(x)
        return x * x_mask


class DilatedDepthSeparableConv(nn.Module):
    """
    Dialted and Depth-Separable Convolution
    """

    def __init__(
        self,
        channels: int,
        kernel_size: int,
        n_layers: int,
        p_dropout: float = 0.0,
    ) -> None:
        super(DilatedDepthSeparableConv, self).__init__()
        self.channels = channels
        self.kernel_size = kernel_size
        self.n_layers = n_layers
        self.p_dropout = p_dropout

        self.drop = nn.Dropout(self.p_dropout)
        self.convs_sep = nn.ModuleList()
        self.convs_1x1 = nn.ModuleList()
        self.norms_1 = nn.ModuleList()
        self.norms_2 = nn.ModuleList()
        for i in range(n_layers):
            dilation = kernel_size**i
            padding = (kernel_size * dilation - dilation) // 2
            self.convs_sep.append(
                nn.Conv1d(
                    channels,
                    channels,
                    kernel_size,
                    groups=channels,
                    dilation=dilation,
                    padding=padding,
                )
            )
            self.convs_1x1.append(nn.Conv1d(channels, channels, 1))
            self.norms_1.append(LayerNorm(channels))
            self.norms_2.append(LayerNorm(channels))

    def forward(
        self, x: torch.Tensor, x_mask: torch.Tensor, g: torch.Tensor | None = None
    ) -> torch.Tensor:
        if g is not None:
            x = x + g
        for i in range(self.n_layers):
            y = self.convs_sep[i](x * x_mask)
            y = self.norms_1[i](y)
            y = F.gelu(y)
            y = self.convs_1x1[i](y)
            y = self.norms_2[i](y)
            y = F.gelu(y)
            y = self.drop(y)
            x = x + y
        return x * x_mask


class WaveNetResidualBlock(torch.nn.Module):
    def __init__(
        self,
        hidden_channels: int,
        kernel_size: int,
        dilation_rate: int,
        n_layers: int,
        gin_channels: int = 0,
        p_dropout: float = 0.0,
    ) -> None:
        super(WaveNetResidualBlock, self).__init__()
        assert kernel_size % 2 == 1
        self.hidden_channels = hidden_channels
        self.kernel_size = (kernel_size,)
        self.dilation_rate = dilation_rate
        self.n_layers = n_layers
        self.gin_channels = gin_channels
        self.p_dropout = p_dropout

        self.in_layers = torch.nn.ModuleList()
        self.res_skip_layers = torch.nn.ModuleList()
        self.drop = nn.Dropout(self.p_dropout)

        if gin_channels != 0:
            cond_layer = torch.nn.Conv1d(
                gin_channels, 2 * hidden_channels * n_layers, 1
            )
            self.cond_layer = weight_norm(cond_layer, name="weight")

        for i in range(n_layers):
            dilation = dilation_rate**i
            padding = int((kernel_size * dilation - dilation) / 2)
            in_layer = torch.nn.Conv1d(
                hidden_channels,
                2 * hidden_channels,
                kernel_size,
                dilation=dilation,
                padding=padding,
            )
            in_layer = weight_norm(in_layer, name="weight")
            self.in_layers.append(in_layer)

            # last one is not necessary
            if i < n_layers - 1:
                res_skip_channels = 2 * hidden_channels
            else:
                res_skip_channels = hidden_channels

            res_skip_layer = torch.nn.Conv1d(hidden_channels, res_skip_channels, 1)
            res_skip_layer = weight_norm(res_skip_layer, name="weight")
            self.res_skip_layers.append(res_skip_layer)

    def forward(
        self, x: torch.Tensor, x_mask: torch.Tensor, g: torch.Tensor | None = None
    ) -> torch.Tensor:
        output = torch.zeros_like(x)
        n_channels_tensor = torch.IntTensor([self.hidden_channels])

        if g is not None:
            g = self.cond_layer(g)

        for i, (in_layer, res_skip_layer) in enumerate(
            zip(self.in_layers, self.res_skip_layers)
        ):
            x_in = in_layer(x)
            if g is not None:
                cond_offset = i * 2 * self.hidden_channels
                g_l = g[:, cond_offset : cond_offset + 2 * self.hidden_channels, :]
            else:
                g_l = torch.zeros_like(x_in)

            acts = utils.fused_add_tanh_sigmoid_multiply(x_in, g_l, n_channels_tensor)
            acts = self.drop(acts)

            res_skip_acts = res_skip_layer(acts)
            if i < self.n_layers - 1:
                res_acts = res_skip_acts[:, : self.hidden_channels, :]
                x = (x + res_acts) * x_mask
                output = output + res_skip_acts[:, self.hidden_channels :, :]
            else:
                output = output + res_skip_acts
        return output * x_mask

    def remove_weight_norm(self) -> None:
        if self.gin_channels != 0:
            remove_weight_norm(self.cond_layer)
        for layer in self.in_layers:
            remove_weight_norm(layer)
        for layer in self.res_skip_layers:
            remove_weight_norm(layer)


WN = WaveNetResidualBlock
DDSConv = DilatedDepthSeparableConv


class ResBlock1(torch.nn.Module):
    def __init__(
        self,
        channels: int,
        kernel_size: int = 3,
        dilation: Sequence[int] = (1, 3, 5),
    ) -> None:
        super(ResBlock1, self).__init__()
        self.convs1 = nn.ModuleList(
            [
                weight_norm(
                    Conv1d(
                        channels,
                        channels,
                        kernel_size,
                        1,
                        dilation=dilation[0],
                        padding=get_padding(kernel_size, dilation[0]),
                    )
                ),
                weight_norm(
                    Conv1d(
                        channels,
                        channels,
                        kernel_size,
                        1,
                        dilation=dilation[1],
                        padding=get_padding(kernel_size, dilation[1]),
                    )
                ),
                weight_norm(
                    Conv1d(
                        channels,
                        channels,
                        kernel_size,
                        1,
                        dilation=dilation[2],
                        padding=get_padding(kernel_size, dilation[2]),
                    )
                ),
            ]
        )
        self.convs1.apply(init_weights)

        self.convs2 = nn.ModuleList(
            [
                weight_norm(
                    Conv1d(
                        channels,
                        channels,
                        kernel_size,
                        1,
                        dilation=1,
                        padding=get_padding(kernel_size, 1),
                    )
                ),
                weight_norm(
                    Conv1d(
                        channels,
                        channels,
                        kernel_size,
                        1,
                        dilation=1,
                        padding=get_padding(kernel_size, 1),
                    )
                ),
                weight_norm(
                    Conv1d(
                        channels,
                        channels,
                        kernel_size,
                        1,
                        dilation=1,
                        padding=get_padding(kernel_size, 1),
                    )
                ),
            ]
        )
        self.convs2.apply(init_weights)
        self.lrelu_slope = LRELU_SLOPE

    def forward(
        self, x: torch.Tensor, x_mask: torch.Tensor | None = None
    ) -> torch.Tensor:
        for c1, c2 in zip(self.convs1, self.convs2):
            xt = F.leaky_relu(x, self.lrelu_slope)
            if x_mask is not None:
                xt = xt * x_mask
            xt = c1(xt)
            xt = F.leaky_relu(xt, self.lrelu_slope)
            if x_mask is not None:
                xt = xt * x_mask
            xt = c2(xt)
            x = xt + x
        if x_mask is not None:
            x = x * x_mask
        return x

    def remove_weight_norm(self) -> None:
        for layer in self.convs1:
            remove_weight_norm(layer)
        for layer in self.convs2:
            remove_weight_norm(layer)


class ResBlock2(torch.nn.Module):
    def __init__(
        self,
        channels: int,
        kernel_size: int = 3,
        dilation: Sequence[int] = (1, 3),
    ) -> None:
        super(ResBlock2, self).__init__()
        self.convs = nn.ModuleList(
            [
                weight_norm(
                    Conv1d(
                        channels,
                        channels,
                        kernel_size,
                        1,
                        dilation=dilation[0],
                        padding=get_padding(kernel_size, dilation[0]),
                    )
                ),
                weight_norm(
                    Conv1d(
                        channels,
                        channels,
                        kernel_size,
                        1,
                        dilation=dilation[1],
                        padding=get_padding(kernel_size, dilation[1]),
                    )
                ),
            ]
        )
        self.convs.apply(init_weights)
        self.lrelu_slope = LRELU_SLOPE

    def forward(
        self, x: torch.Tensor, x_mask: torch.Tensor | None = None
    ) -> torch.Tensor:
        for c in self.convs:
            xt = F.leaky_relu(x, self.lrelu_slope)
            if x_mask is not None:
                xt = xt * x_mask
            xt = c(xt)
            x = xt + x
        if x_mask is not None:
            x = x * x_mask
        return x

    def remove_weight_norm(self) -> None:
        for layer in self.convs:
            remove_weight_norm(layer)
