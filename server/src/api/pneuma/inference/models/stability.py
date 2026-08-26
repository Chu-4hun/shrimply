import torch


def make_tensor_stable(x: torch.Tensor) -> torch.Tensor:
    x = torch.nan_to_num(x, nan=0.0, posinf=10.0, neginf=-10.0)
    return x.clamp(min=-10.0, max=10.0)
