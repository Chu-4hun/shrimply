import math
from collections import OrderedDict
from dataclasses import dataclass

import torch
from torch import Tensor, nn
from torch.nn import functional as F

from api.tts.index_cuda_graph import CudaGraphCache, replay_cuda_graph


@dataclass(frozen=True, slots=True)
class VocoderConfig:
    mel_channels: int = 80
    initial_channels: int = 1_536
    upsample_rates: tuple[int, ...] = (4, 4, 2, 2, 2, 2)
    upsample_kernel_sizes: tuple[int, ...] = (8, 8, 4, 4, 4, 4)
    residual_kernel_sizes: tuple[int, ...] = (3, 7, 11)
    residual_dilations: tuple[tuple[int, ...], ...] = (
        (1, 3, 5),
        (1, 3, 5),
        (1, 3, 5),
    )
    snake_logscale: bool = True
    use_final_tanh: bool = False
    final_bias: bool = False

    def validate(self) -> None:
        if len(self.upsample_rates) != len(self.upsample_kernel_sizes):
            raise ValueError("Vocoder upsample rates and kernels must have equal lengths")
        if len(self.residual_kernel_sizes) != len(self.residual_dilations):
            raise ValueError("Vocoder residual kernels and dilations must have equal lengths")


class WeightNormConv1d(nn.Module):
    def __init__(
        self,
        input_channels: int,
        output_channels: int,
        kernel_size: int,
        padding: int,
        dilation: int = 1,
        bias: bool = True,
    ) -> None:
        super().__init__()
        weight = torch.empty(output_channels, input_channels, kernel_size)
        nn.init.kaiming_uniform_(weight, a=math.sqrt(5))
        self.weight_v = nn.Parameter(weight)
        self.weight_g = nn.Parameter(
            torch.linalg.vector_norm(weight, dim=(1, 2), keepdim=True)
        )
        if bias:
            self.bias: nn.Parameter | None = nn.Parameter(torch.empty(output_channels))
            bound = 1 / math.sqrt(input_channels * kernel_size)
            nn.init.uniform_(self.bias, -bound, bound)
        else:
            self.register_parameter("bias", None)
        self._padding = padding
        self._dilation = dilation
        self.weight_precomputed = False

    def forward(self, inputs: Tensor) -> Tensor:
        if self.weight_precomputed:
            weight = self.weight_v
        else:
            norm = torch.linalg.vector_norm(self.weight_v, dim=(1, 2), keepdim=True)
            weight = self.weight_v * self.weight_g / norm
        return F.conv1d(
            inputs,
            weight,
            self.bias,
            padding=self._padding,
            dilation=self._dilation,
        )


class WeightNormConvTranspose1d(nn.Module):
    def __init__(
        self,
        input_channels: int,
        output_channels: int,
        kernel_size: int,
        stride: int,
        padding: int,
    ) -> None:
        super().__init__()
        weight = torch.empty(input_channels, output_channels, kernel_size)
        nn.init.kaiming_uniform_(weight, a=math.sqrt(5))
        self.weight_v = nn.Parameter(weight)
        self.weight_g = nn.Parameter(
            torch.linalg.vector_norm(weight, dim=(1, 2), keepdim=True)
        )
        self.bias = nn.Parameter(torch.empty(output_channels))
        bound = 1 / math.sqrt(input_channels * kernel_size)
        nn.init.uniform_(self.bias, -bound, bound)
        self._stride = stride
        self._padding = padding
        self.weight_precomputed = False

    def forward(self, inputs: Tensor) -> Tensor:
        if self.weight_precomputed:
            weight = self.weight_v
        else:
            norm = torch.linalg.vector_norm(self.weight_v, dim=(1, 2), keepdim=True)
            weight = self.weight_v * self.weight_g / norm
        return F.conv_transpose1d(
            inputs,
            weight,
            self.bias,
            stride=self._stride,
            padding=self._padding,
        )


class SnakeBeta(nn.Module):
    def __init__(self, channels: int, logarithmic: bool) -> None:
        super().__init__()
        initial = torch.zeros(channels) if logarithmic else torch.ones(channels)
        self.alpha = nn.Parameter(initial.clone())
        self.beta = nn.Parameter(initial)
        self.alpha_logscale = logarithmic
        self.use_triton = False

    def forward(self, values: Tensor) -> Tensor:
        if self.use_triton:
            from api.tts.index_triton import snake_beta

            return snake_beta(values, self.alpha, self.beta, self.alpha_logscale)
        alpha = self.alpha[None, :, None]
        beta = self.beta[None, :, None]
        if self.alpha_logscale:
            alpha = torch.exp(alpha)
            beta = torch.exp(beta)
        return values + torch.sin(values * alpha).square() / (beta + 1e-9)


def _kaiser_sinc_filter(cutoff: float, half_width: float, size: int) -> Tensor:
    half_size = size // 2
    attenuation = 2.285 * (half_size - 1) * math.pi * 4 * half_width + 7.95
    if attenuation > 50:
        beta = 0.1102 * (attenuation - 8.7)
    elif attenuation >= 21:
        beta = 0.5842 * (attenuation - 21) ** 0.4 + 0.07886 * (
            attenuation - 21
        )
    else:
        beta = 0.0
    window = torch.kaiser_window(size, beta=beta, periodic=False)
    if size % 2 == 0:
        time = torch.arange(-half_size, half_size) + 0.5
    else:
        time = torch.arange(size) - half_size
    result = 2 * cutoff * window * torch.sinc(2 * cutoff * time)
    return (result / result.sum()).view(1, 1, size)


class UpSample1d(nn.Module):
    filter: Tensor

    def __init__(self, ratio: int = 2, kernel_size: int = 12) -> None:
        super().__init__()
        self.ratio = ratio
        self.kernel_size = kernel_size
        self.stride = ratio
        self.pad = kernel_size // ratio - 1
        self.pad_left = self.pad * ratio + (kernel_size - ratio) // 2
        self.pad_right = self.pad * ratio + (kernel_size - ratio + 1) // 2
        self.register_buffer(
            "filter",
            _kaiser_sinc_filter(0.5 / ratio, 0.6 / ratio, kernel_size),
        )

    def forward(self, values: Tensor) -> Tensor:
        channels = values.shape[1]
        values = F.pad(values, (self.pad, self.pad), mode="replicate")
        values = self.ratio * F.conv_transpose1d(
            values,
            self.filter.expand(channels, -1, -1),
            stride=self.stride,
            groups=channels,
        )
        return values[..., self.pad_left : -self.pad_right]


class LowPassFilter1d(nn.Module):
    filter: Tensor

    def __init__(self, ratio: int = 2, kernel_size: int = 12) -> None:
        super().__init__()
        even = kernel_size % 2 == 0
        self.pad_left = kernel_size // 2 - int(even)
        self.pad_right = kernel_size // 2
        self.stride = ratio
        self.register_buffer(
            "filter",
            _kaiser_sinc_filter(0.5 / ratio, 0.6 / ratio, kernel_size),
        )

    def forward(self, values: Tensor) -> Tensor:
        channels = values.shape[1]
        values = F.pad(
            values, (self.pad_left, self.pad_right), mode="replicate"
        )
        return F.conv1d(
            values,
            self.filter.expand(channels, -1, -1),
            stride=self.stride,
            groups=channels,
        )


class DownSample1d(nn.Module):
    def __init__(self, ratio: int = 2, kernel_size: int = 12) -> None:
        super().__init__()
        self.ratio = ratio
        self.kernel_size = kernel_size
        self.lowpass = LowPassFilter1d(ratio, kernel_size)

    def forward(self, values: Tensor) -> Tensor:
        return self.lowpass(values)


class Activation1d(nn.Module):
    def __init__(self, channels: int, logarithmic: bool) -> None:
        super().__init__()
        self.up_ratio = 2
        self.down_ratio = 2
        self.act = SnakeBeta(channels, logarithmic)
        self.upsample = UpSample1d()
        self.downsample = DownSample1d()

    def forward(self, values: Tensor) -> Tensor:
        return self.downsample(self.act(self.upsample(values)))


class AMPBlock1(nn.Module):
    def __init__(
        self,
        channels: int,
        kernel_size: int,
        dilations: tuple[int, ...],
        logarithmic: bool,
    ) -> None:
        super().__init__()
        self.convs1 = nn.ModuleList(
            WeightNormConv1d(
                channels,
                channels,
                kernel_size,
                padding=(kernel_size * dilation - dilation) // 2,
                dilation=dilation,
            )
            for dilation in dilations
        )
        self.convs2 = nn.ModuleList(
            WeightNormConv1d(
                channels,
                channels,
                kernel_size,
                padding=(kernel_size - 1) // 2,
            )
            for _ in dilations
        )
        self.activations = nn.ModuleList(
            Activation1d(channels, logarithmic) for _ in range(2 * len(dilations))
        )

    def forward(self, values: Tensor) -> Tensor:
        for index in range(len(self.convs1)):
            first = self.activations[2 * index]
            second = self.activations[2 * index + 1]
            if not isinstance(first, Activation1d) or not isinstance(
                second, Activation1d
            ):
                raise TypeError("Vocoder has an invalid activation")
            convolved = self.convs1[index](first(values))
            values = values + self.convs2[index](second(convolved))
        return values


class Vocoder(nn.Module):
    def __init__(self, config: VocoderConfig = VocoderConfig()) -> None:
        super().__init__()
        config.validate()
        self.num_kernels = len(config.residual_kernel_sizes)
        self.num_upsamples = len(config.upsample_rates)
        self.conv_pre = WeightNormConv1d(
            config.mel_channels,
            config.initial_channels,
            kernel_size=7,
            padding=3,
        )
        self.ups = nn.ModuleList()
        self.resblocks = nn.ModuleList()
        channels = config.initial_channels
        for rate, kernel_size in zip(
            config.upsample_rates, config.upsample_kernel_sizes, strict=True
        ):
            next_channels = channels // 2
            self.ups.append(
                nn.ModuleList(
                    (
                        WeightNormConvTranspose1d(
                            channels,
                            next_channels,
                            kernel_size,
                            rate,
                            padding=(kernel_size - rate) // 2,
                        ),
                    )
                )
            )
            for residual_kernel, dilations in zip(
                config.residual_kernel_sizes,
                config.residual_dilations,
                strict=True,
            ):
                self.resblocks.append(
                    AMPBlock1(
                        next_channels,
                        residual_kernel,
                        dilations,
                        config.snake_logscale,
                    )
                )
            channels = next_channels
        self.activation_post = Activation1d(channels, config.snake_logscale)
        self.use_bias_at_final = config.final_bias
        self.conv_post = WeightNormConv1d(
            channels,
            1,
            kernel_size=7,
            padding=3,
            bias=config.final_bias,
        )
        self.use_tanh_at_final = config.use_final_tanh
        self.use_cuda_graph = False
        self._cuda_graphs: CudaGraphCache = OrderedDict()

    def forward(self, mel: Tensor) -> Tensor:
        if self.use_cuda_graph:
            return replay_cuda_graph(self._cuda_graphs, self._render, (mel,)).clone()
        return self._render(mel)

    def _render(self, mel: Tensor) -> Tensor:
        values = self.conv_pre(mel)
        for index in range(self.num_upsamples):
            upsamplers = self.ups[index]
            if not isinstance(upsamplers, nn.ModuleList):
                raise TypeError("Vocoder has an invalid upsampling stage")
            values = upsamplers[0](values)
            combined: Tensor | None = None
            for kernel in range(self.num_kernels):
                block = self.resblocks[index * self.num_kernels + kernel]
                rendered = block(values)
                combined = rendered if combined is None else combined + rendered
            if combined is None:
                raise RuntimeError("Vocoder stage has no residual blocks")
            values = combined / self.num_kernels
        values = self.conv_post(self.activation_post(values))
        return torch.tanh(values) if self.use_tanh_at_final else values.clamp(-1, 1)
