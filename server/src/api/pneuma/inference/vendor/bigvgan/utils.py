# Adapted from https://github.com/jik876/hifi-gan under the MIT license.
#   LICENSE is in incl_licenses directory.

import torch


def init_weights(m: torch.nn.Module, mean: float = 0.0, std: float = 0.01) -> None:
    if "Conv" in m.__class__.__name__:
        weight = getattr(m, "weight", None)
        if isinstance(weight, torch.Tensor):
            torch.nn.init.normal_(weight, mean, std)


def get_padding(kernel_size: int, dilation: int = 1) -> int:
    return (kernel_size * dilation - dilation) // 2
