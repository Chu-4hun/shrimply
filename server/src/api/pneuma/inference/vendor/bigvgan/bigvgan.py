# Copyright (c) 2024 NVIDIA CORPORATION.
#   Licensed under the MIT license.

# Adapted from https://github.com/jik876/hifi-gan under the MIT license.
#   LICENSE is in incl_licenses directory.

import torch
import torch.nn as nn
from torch.nn import Conv1d
from torch.nn.utils.parametrize import remove_parametrizations
from torch.nn.utils.parametrizations import weight_norm

from . import activations
from .utils import init_weights, get_padding
from .alias_free_activation.torch.act import Activation1d as TorchActivation1d
from .env import AttrDict


def remove_weight_norm(module: nn.Module, name: str = "weight") -> nn.Module:
    remove_parametrizations(module, name, leave_parametrized=True)
    return module


def _make_periodic_activation(
    activation: str | None, channels: int, h: AttrDict
) -> nn.Module:
    if activation == "snake":
        return activations.Snake(channels, alpha_logscale=h.snake_logscale)
    if activation == "snakebeta":
        return activations.SnakeBeta(channels, alpha_logscale=h.snake_logscale)
    raise NotImplementedError(
        "activation incorrectly specified. check the config file and look for 'activation'."
    )


def _make_activation1d(activation: nn.Module, use_cuda_kernel: bool) -> nn.Module:
    if use_cuda_kernel:
        import importlib

        activation1d_module = importlib.import_module(
            ".alias_free_activation.cuda.activation1d", package=__package__
        )
        return activation1d_module.Activation1d(activation=activation)
    return TorchActivation1d(activation=activation)


class AMPBlock1(torch.nn.Module):
    """
    AMPBlock applies Snake / SnakeBeta activation functions with trainable parameters that control periodicity, defined for each layer.
    AMPBlock1 has additional self.convs2 that contains additional Conv1d layers with a fixed dilation=1 followed by each layer in self.convs1

    Args:
        h (AttrDict): Hyperparameters.
        channels (int): Number of convolution channels.
        kernel_size (int): Size of the convolution kernel. Default is 3.
        dilation (tuple): Dilation rates for the convolutions. Each dilation layer has two convolutions. Default is (1, 3, 5).
        activation (str): Activation function type. Should be either 'snake' or 'snakebeta'. Default is None.
    """

    def __init__(
        self,
        h: AttrDict,
        channels: int,
        kernel_size: int = 3,
        dilation: tuple[int, int, int] = (1, 3, 5),
        activation: str | None = None,
    ) -> None:
        super().__init__()
        if len(dilation) != 3:
            raise ValueError("AMPBlock1 expects exactly three dilation rates.")

        self.h = h

        self.convs1 = nn.ModuleList(
            [
                weight_norm(
                    Conv1d(
                        channels,
                        channels,
                        kernel_size,
                        stride=1,
                        dilation=d,
                        padding=get_padding(kernel_size, d),
                    )
                )
                for d in dilation
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
                        stride=1,
                        dilation=1,
                        padding=get_padding(kernel_size, 1),
                    )
                )
                for _ in range(len(dilation))
            ]
        )
        self.convs2.apply(init_weights)

        self.num_layers = len(self.convs1) + len(
            self.convs2
        )  # Total number of conv layers

        self.activations = nn.ModuleList(
            [
                _make_activation1d(
                    _make_periodic_activation(activation, channels, h),
                    use_cuda_kernel=self.h.get("use_cuda_kernel", False),
                )
                for _ in range(self.num_layers)
            ]
        )

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        xt = self.activations[0](x)
        xt = self.convs1[0](xt)
        xt = self.activations[1](xt)
        xt = self.convs2[0](xt)
        x = xt + x

        xt = self.activations[2](x)
        xt = self.convs1[1](xt)
        xt = self.activations[3](xt)
        xt = self.convs2[1](xt)
        x = xt + x

        xt = self.activations[4](x)
        xt = self.convs1[2](xt)
        xt = self.activations[5](xt)
        xt = self.convs2[2](xt)
        x = xt + x

        return x

    def remove_weight_norm(self) -> None:
        for layer in self.convs1:
            remove_weight_norm(layer)
        for layer in self.convs2:
            remove_weight_norm(layer)


class AMPBlock2(torch.nn.Module):
    """
    AMPBlock applies Snake / SnakeBeta activation functions with trainable parameters that control periodicity, defined for each layer.
    Unlike AMPBlock1, AMPBlock2 does not contain extra Conv1d layers with fixed dilation=1

    Args:
        h (AttrDict): Hyperparameters.
        channels (int): Number of convolution channels.
        kernel_size (int): Size of the convolution kernel. Default is 3.
        dilation (tuple): Dilation rates for the convolutions. Each dilation layer has two convolutions. Default is (1, 3, 5).
        activation (str): Activation function type. Should be either 'snake' or 'snakebeta'. Default is None.
    """

    def __init__(
        self,
        h: AttrDict,
        channels: int,
        kernel_size: int = 3,
        dilation: tuple[int, ...] = (1, 3, 5),
        activation: str | None = None,
    ) -> None:
        super().__init__()

        self.h = h

        self.convs = nn.ModuleList(
            [
                weight_norm(
                    Conv1d(
                        channels,
                        channels,
                        kernel_size,
                        stride=1,
                        dilation=d,
                        padding=get_padding(kernel_size, d),
                    )
                )
                for d in dilation
            ]
        )
        self.convs.apply(init_weights)

        self.num_layers = len(self.convs)  # Total number of conv layers

        self.activations = nn.ModuleList(
            [
                _make_activation1d(
                    _make_periodic_activation(activation, channels, h),
                    use_cuda_kernel=self.h.get("use_cuda_kernel", False),
                )
                for _ in range(self.num_layers)
            ]
        )

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        for c, a in zip(self.convs, self.activations):
            xt = a(x)
            xt = c(xt)
            x = xt + x
        return x

    def remove_weight_norm(self) -> None:
        for layer in self.convs:
            remove_weight_norm(layer)
