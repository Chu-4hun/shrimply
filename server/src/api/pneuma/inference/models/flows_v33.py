import torch
from torch import nn
from torch.nn import functional as F

from api.pneuma.inference.models.blocks import RMSNorm
from api.pneuma.inference.models.flows import Flip

V33_FLOW_LAYER_SCALE_INIT = 0.01


class V33FlowResidualBlock(nn.Module):
    def __init__(self, channels: int, kernel_size: int, dilation: int) -> None:
        super().__init__()
        if kernel_size % 2 != 1:
            raise ValueError("Flow residual kernel size must be odd.")
        padding = (kernel_size * dilation - dilation) // 2
        self.norm = RMSNorm(channels)
        self.depthwise = nn.Conv1d(
            channels,
            channels,
            kernel_size,
            dilation=dilation,
            padding=padding,
            groups=channels,
        )
        self.gate = nn.Conv1d(channels, channels * 4, 1)
        self.up = nn.Conv1d(channels, channels * 4, 1)
        self.down = nn.Conv1d(channels * 4, channels, 1)
        self.layer_scale = nn.Parameter(
            torch.full((channels, 1), V33_FLOW_LAYER_SCALE_INIT)
        )

    def forward(self, x: torch.Tensor, x_mask: torch.Tensor) -> torch.Tensor:
        residual = x
        y = self.norm(x)
        y = self.depthwise(y * x_mask)
        y = F.silu(self.gate(y)) * self.up(y)
        y = self.down(y)
        return (residual + self.layer_scale.unsqueeze(0) * y) * x_mask


class V33FlowBackbone(nn.Module):
    def __init__(
        self,
        channels: int,
        kernel_size: int,
        n_layers: int,
    ) -> None:
        super().__init__()
        self.blocks = nn.ModuleList(
            [
                V33FlowResidualBlock(
                    channels,
                    kernel_size,
                    dilation=kernel_size**layer_index,
                )
                for layer_index in range(n_layers)
            ]
        )

    def forward(self, x: torch.Tensor, x_mask: torch.Tensor) -> torch.Tensor:
        x = x * x_mask
        for block in self.blocks:
            x = block(x, x_mask)
        return x * x_mask


class V33ResidualCouplingLayer(nn.Module):
    def __init__(
        self,
        channels: int,
        hidden_channels: int,
        kernel_size: int,
        n_layers: int,
    ) -> None:
        super().__init__()
        if channels % 2 != 0:
            raise ValueError("channels must be divisible by 2.")
        self.half_channels = channels // 2
        self.pre = nn.Conv1d(self.half_channels, hidden_channels, 1)
        self.backbone = V33FlowBackbone(hidden_channels, kernel_size, n_layers)
        self.post = nn.Conv1d(hidden_channels, self.half_channels, 1)
        nn.init.zeros_(self.post.weight)
        if self.post.bias is not None:
            nn.init.zeros_(self.post.bias)

    def forward(
        self,
        x: torch.Tensor,
        x_mask: torch.Tensor,
        reverse: bool = False,
    ) -> tuple[torch.Tensor, torch.Tensor]:
        x0, x1 = torch.split(x, [self.half_channels] * 2, dim=1)
        h = self.pre(x0) * x_mask
        h = self.backbone(h, x_mask)
        m = self.post(h) * x_mask
        if reverse:
            x1 = (x1 - m) * x_mask
            x = torch.cat([x0, x1], dim=1) * x_mask
            return x, x.new_zeros(x.shape[0])
        x1 = (m + x1) * x_mask
        x = torch.cat([x0, x1], dim=1) * x_mask
        return x, x.new_zeros(x.shape[0])

    def remove_weight_norm(self) -> None:
        return None


class V33ResidualCouplingBlock(nn.Module):
    def __init__(
        self,
        channels: int,
        hidden_channels: int,
        kernel_size: int,
        n_layers: int,
        n_flows: int = 4,
    ) -> None:
        super().__init__()
        self.n_flows = n_flows
        self.flows = nn.ModuleList()
        for _ in range(n_flows):
            self.flows.append(
                V33ResidualCouplingLayer(
                    channels,
                    hidden_channels,
                    kernel_size,
                    n_layers,
                )
            )
            self.flows.append(Flip())

    def forward(
        self,
        x: torch.Tensor,
        x_mask: torch.Tensor,
        reverse: bool = False,
    ) -> torch.Tensor:
        flows = reversed(self.flows) if reverse else self.flows
        for flow in flows:
            if isinstance(flow, Flip):
                x, _ = flow(x)
            else:
                x, _ = flow(x, x_mask, reverse=reverse)
        return x * x_mask

    def remove_weight_norm(self) -> None:
        for flow in self.flows:
            if isinstance(flow, V33ResidualCouplingLayer):
                flow.remove_weight_norm()
