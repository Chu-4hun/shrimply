from typing import Literal

import torch
from torch import Tensor, nn
from torch.nn import functional as F

type Nonlinearity = Literal["batchnorm-relu", "batchnorm_"]


def _nonlinearity(kind: Nonlinearity, channels: int) -> nn.Sequential:
    modules = nn.Sequential()
    if kind == "batchnorm_":
        modules.add_module("batchnorm", nn.BatchNorm1d(channels, affine=False))
        return modules
    modules.add_module("batchnorm", nn.BatchNorm1d(channels))
    modules.add_module("relu", nn.ReLU(inplace=True))
    return modules


class BasicResBlock(nn.Module):
    expansion = 1

    def __init__(self, input_channels: int, channels: int, stride: int = 1) -> None:
        super().__init__()
        self.conv1 = nn.Conv2d(
            input_channels,
            channels,
            kernel_size=3,
            stride=(stride, 1),
            padding=1,
            bias=False,
        )
        self.bn1 = nn.BatchNorm2d(channels)
        self.conv2 = nn.Conv2d(
            channels, channels, kernel_size=3, padding=1, bias=False
        )
        self.bn2 = nn.BatchNorm2d(channels)
        if stride != 1 or input_channels != channels:
            self.shortcut = nn.Sequential(
                nn.Conv2d(
                    input_channels,
                    channels,
                    kernel_size=1,
                    stride=(stride, 1),
                    bias=False,
                ),
                nn.BatchNorm2d(channels),
            )
        else:
            self.shortcut = nn.Sequential()

    def forward(self, values: Tensor) -> Tensor:
        residual = self.shortcut(values)
        values = F.relu(self.bn1(self.conv1(values)))
        return F.relu(self.bn2(self.conv2(values)) + residual)


class FCM(nn.Module):
    def __init__(
        self,
        feature_dimension: int,
        channels: int = 32,
        blocks_per_layer: tuple[int, int] = (2, 2),
    ) -> None:
        super().__init__()
        self.conv1 = nn.Conv2d(
            1, channels, kernel_size=3, padding=1, bias=False
        )
        self.bn1 = nn.BatchNorm2d(channels)
        self.layer1 = self._layer(channels, channels, blocks_per_layer[0], 2)
        self.layer2 = self._layer(channels, channels, blocks_per_layer[1], 2)
        self.conv2 = nn.Conv2d(
            channels,
            channels,
            kernel_size=3,
            stride=(2, 1),
            padding=1,
            bias=False,
        )
        self.bn2 = nn.BatchNorm2d(channels)
        self.out_channels = channels * (feature_dimension // 8)

    @staticmethod
    def _layer(
        input_channels: int,
        channels: int,
        blocks: int,
        stride: int,
    ) -> nn.Sequential:
        return nn.Sequential(
            BasicResBlock(input_channels, channels, stride),
            *(BasicResBlock(channels, channels) for _ in range(blocks - 1)),
        )

    def forward(self, values: Tensor) -> Tensor:
        values = values.unsqueeze(1)
        values = F.relu(self.bn1(self.conv1(values)))
        values = self.layer1(values)
        values = self.layer2(values)
        values = F.relu(self.bn2(self.conv2(values)))
        batch, channels, frequency, time = values.shape
        return values.reshape(batch, channels * frequency, time)


class TDNNLayer(nn.Module):
    def __init__(
        self,
        input_channels: int,
        output_channels: int,
        kernel_size: int,
        stride: int = 1,
        dilation: int = 1,
        bias: bool = False,
        nonlinearity: Nonlinearity = "batchnorm-relu",
    ) -> None:
        super().__init__()
        padding = (kernel_size - 1) // 2 * dilation
        self.linear = nn.Conv1d(
            input_channels,
            output_channels,
            kernel_size,
            stride=stride,
            padding=padding,
            dilation=dilation,
            bias=bias,
        )
        self.nonlinear = _nonlinearity(nonlinearity, output_channels)

    def forward(self, values: Tensor) -> Tensor:
        return self.nonlinear(self.linear(values))


class CAMLayer(nn.Module):
    def __init__(
        self,
        bottleneck_channels: int,
        output_channels: int,
        kernel_size: int,
        dilation: int,
        reduction: int = 2,
    ) -> None:
        super().__init__()
        padding = (kernel_size - 1) // 2 * dilation
        self.linear_local = nn.Conv1d(
            bottleneck_channels,
            output_channels,
            kernel_size,
            padding=padding,
            dilation=dilation,
            bias=False,
        )
        self.linear1 = nn.Conv1d(
            bottleneck_channels, bottleneck_channels // reduction, 1
        )
        self.relu = nn.ReLU(inplace=True)
        self.linear2 = nn.Conv1d(
            bottleneck_channels // reduction, output_channels, 1
        )
        self.sigmoid = nn.Sigmoid()

    def forward(self, values: Tensor) -> Tensor:
        local = self.linear_local(values)
        pooled = F.avg_pool1d(
            values, kernel_size=100, stride=100, ceil_mode=True
        )
        shape = pooled.shape
        pooled = pooled.unsqueeze(-1).expand(*shape, 100).reshape(*shape[:-1], -1)
        context = values.mean(-1, keepdim=True) + pooled[..., : values.shape[-1]]
        gate = self.sigmoid(self.linear2(self.relu(self.linear1(context))))
        return local * gate


class CAMDenseTDNNLayer(nn.Module):
    def __init__(
        self,
        input_channels: int,
        output_channels: int,
        bottleneck_channels: int,
        kernel_size: int,
        dilation: int,
    ) -> None:
        super().__init__()
        self.nonlinear1 = _nonlinearity("batchnorm-relu", input_channels)
        self.linear1 = nn.Conv1d(
            input_channels, bottleneck_channels, 1, bias=False
        )
        self.nonlinear2 = _nonlinearity("batchnorm-relu", bottleneck_channels)
        self.cam_layer = CAMLayer(
            bottleneck_channels, output_channels, kernel_size, dilation
        )

    def forward(self, values: Tensor) -> Tensor:
        values = self.linear1(self.nonlinear1(values))
        return self.cam_layer(self.nonlinear2(values))


class CAMDenseTDNNBlock(nn.ModuleList):
    def __init__(
        self,
        layers: int,
        input_channels: int,
        output_channels: int,
        bottleneck_channels: int,
        kernel_size: int,
        dilation: int,
    ) -> None:
        super().__init__()
        for index in range(layers):
            self.add_module(
                f"tdnnd{index + 1}",
                CAMDenseTDNNLayer(
                    input_channels + index * output_channels,
                    output_channels,
                    bottleneck_channels,
                    kernel_size,
                    dilation,
                ),
            )

    def forward(self, values: Tensor) -> Tensor:
        for module in self:
            if not isinstance(module, CAMDenseTDNNLayer):
                raise TypeError("Speaker encoder has an invalid TDNN layer")
            layer = module
            values = torch.cat((values, layer(values)), dim=1)
        return values


class TransitLayer(nn.Module):
    def __init__(self, input_channels: int, output_channels: int) -> None:
        super().__init__()
        self.nonlinear = _nonlinearity("batchnorm-relu", input_channels)
        self.linear = nn.Conv1d(input_channels, output_channels, 1, bias=False)

    def forward(self, values: Tensor) -> Tensor:
        return self.linear(self.nonlinear(values))


class StatsPool(nn.Module):
    def forward(self, values: Tensor) -> Tensor:
        return torch.cat(
            (values.mean(dim=-1), values.std(dim=-1, unbiased=True)), dim=-1
        )


class DenseLayer(nn.Module):
    def __init__(self, input_channels: int, output_channels: int) -> None:
        super().__init__()
        self.linear = nn.Conv1d(input_channels, output_channels, 1, bias=False)
        self.nonlinear = _nonlinearity("batchnorm_", output_channels)

    def forward(self, values: Tensor) -> Tensor:
        values = self.linear(values.unsqueeze(-1)).squeeze(-1)
        return self.nonlinear(values)


class SpeakerEncoder(nn.Module):
    def __init__(
        self,
        feature_dimension: int = 80,
        embedding_dimension: int = 192,
        growth_rate: int = 32,
        bottleneck_multiplier: int = 4,
        initial_channels: int = 128,
    ) -> None:
        super().__init__()
        self.head = FCM(feature_dimension)
        channels = self.head.out_channels
        self.xvector = nn.Sequential()
        self.xvector.add_module(
            "tdnn",
            TDNNLayer(
                channels,
                initial_channels,
                kernel_size=5,
                stride=2,
            ),
        )
        channels = initial_channels
        for index, (layers, dilation) in enumerate(((12, 1), (24, 2), (16, 2))):
            self.xvector.add_module(
                f"block{index + 1}",
                CAMDenseTDNNBlock(
                    layers,
                    channels,
                    growth_rate,
                    bottleneck_multiplier * growth_rate,
                    kernel_size=3,
                    dilation=dilation,
                ),
            )
            channels += layers * growth_rate
            self.xvector.add_module(
                f"transit{index + 1}", TransitLayer(channels, channels // 2)
            )
            channels //= 2
        self.xvector.add_module(
            "out_nonlinear", _nonlinearity("batchnorm-relu", channels)
        )
        self.xvector.add_module("stats", StatsPool())
        self.xvector.add_module(
            "dense", DenseLayer(2 * channels, embedding_dimension)
        )

    def forward(self, features: Tensor) -> Tensor:
        return self.xvector(self.head(features.permute(0, 2, 1)))
