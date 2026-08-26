import math

import torch
from torch import nn
from torch.nn import functional as F

from api.pneuma.inference.models.attentions_v32 import V32Attention, V32SwiGLUFFN
from api.pneuma.inference.models.blocks import RMSNorm
from api.pneuma.inference.models.stability import make_tensor_stable

V32_CFM_LAYERS = 4
V32_CFM_STEPS = 4
V32_TIME_EMBED_CHANNELS = 256


class V32TimestepEmbedding(nn.Module):
    def __init__(
        self, channels: int, time_channels: int = V32_TIME_EMBED_CHANNELS
    ) -> None:
        super().__init__()
        self.time_channels = time_channels
        self.proj = nn.Sequential(
            nn.Linear(time_channels, channels),
            nn.SiLU(),
            nn.Linear(channels, channels),
        )

    def forward(self, t: torch.Tensor) -> torch.Tensor:
        half_channels = self.time_channels // 2
        scale = math.log(10000.0) / (half_channels - 1)
        freqs = torch.exp(
            torch.arange(half_channels, device=t.device, dtype=torch.float32) * -scale
        )
        args = t.float().unsqueeze(1) * freqs.unsqueeze(0)
        embedding = torch.cat([args.sin(), args.cos()], dim=-1)
        target_dtype: torch.dtype = next(self.proj.parameters()).dtype
        return self.proj(embedding.to(dtype=target_dtype)).to(dtype=t.dtype)


class V32AdaRMSNorm(nn.Module):
    def __init__(self, channels: int) -> None:
        super().__init__()
        self.norm = RMSNorm(channels)
        self.modulation = nn.Sequential(
            nn.SiLU(),
            nn.Linear(channels, channels * 3),
        )

    def forward(
        self, x: torch.Tensor, time_embedding: torch.Tensor
    ) -> tuple[torch.Tensor, torch.Tensor]:
        target_dtype: torch.dtype = next(self.modulation.parameters()).dtype
        shift, scale, gate = self.modulation(
            time_embedding.to(dtype=target_dtype)
        ).chunk(3, dim=1)
        shift = shift.to(dtype=x.dtype)
        scale = scale.to(dtype=x.dtype)
        gate = gate.to(dtype=x.dtype)
        x = self.norm(x)
        x = x * (1.0 + scale.unsqueeze(-1)) + shift.unsqueeze(-1)
        return x, gate.unsqueeze(-1)


class V32CFMBlock(nn.Module):
    def __init__(self, channels: int, n_heads: int, kernel_size: int) -> None:
        super().__init__()
        self.attn_norm = V32AdaRMSNorm(channels)
        self.attn = V32Attention(channels, n_heads)
        self.ffn_norm = V32AdaRMSNorm(channels)
        self.ffn = V32SwiGLUFFN(channels, kernel_size)

    def forward(
        self, x: torch.Tensor, x_mask: torch.Tensor, time_embedding: torch.Tensor
    ) -> torch.Tensor:
        y, gate = self.attn_norm(x, time_embedding)
        x = x + gate * self.attn(y, x_mask)
        y, gate = self.ffn_norm(x, time_embedding)
        x = x + gate * self.ffn(y, x_mask)
        return x * x_mask


def masked_mse(
    prediction: torch.Tensor,
    target: torch.Tensor,
    mask: torch.Tensor,
) -> torch.Tensor:
    loss = F.mse_loss(prediction.float(), target.float(), reduction="none")
    loss = loss * mask.float()
    denom = mask.float().sum() * prediction.shape[1]
    return loss.sum() / denom.clamp_min(1.0)


class V32LatentCFM(nn.Module):
    def __init__(
        self,
        channels: int,
        n_heads: int,
        kernel_size: int,
        n_layers: int = V32_CFM_LAYERS,
    ) -> None:
        super().__init__()
        self.channels = channels
        self.time_embed = V32TimestepEmbedding(channels)
        self.input_proj = nn.Conv1d(channels * 2, channels, 1)
        self.layers = nn.ModuleList(
            [V32CFMBlock(channels, n_heads, kernel_size) for _ in range(n_layers)]
        )
        self.out_norm = RMSNorm(channels)
        self.out_proj = nn.Conv1d(channels, channels, 1)
        nn.init.zeros_(self.out_proj.weight)
        if self.out_proj.bias is not None:
            nn.init.zeros_(self.out_proj.bias)

    def forward(
        self,
        x_t: torch.Tensor,
        condition: torch.Tensor,
        x_mask: torch.Tensor,
        t: torch.Tensor,
    ) -> torch.Tensor:
        time_embedding = self.time_embed(t)
        x = self.input_proj(torch.cat([x_t, condition], dim=1)) * x_mask
        for layer in self.layers:
            x = layer(x, x_mask, time_embedding)
        return self.out_proj(self.out_norm(x)) * x_mask

    def train_step(
        self,
        content_latent: torch.Tensor,
        target_latent: torch.Tensor,
        x_mask: torch.Tensor,
        *,
        numeric_stability_check: bool = False,
    ) -> tuple[torch.Tensor, torch.Tensor]:
        batch_size = content_latent.shape[0]
        t = torch.rand(
            batch_size,
            device=content_latent.device,
            dtype=content_latent.dtype,
        )
        t_view = t.view(batch_size, 1, 1)
        base = content_latent.detach()
        target = target_latent.detach()
        condition = content_latent
        if numeric_stability_check:
            base = make_tensor_stable(base)
            target = make_tensor_stable(target)
            condition = make_tensor_stable(condition)
        x_t = ((1.0 - t_view) * base + t_view * target) * x_mask
        velocity_target = (target - base) * x_mask
        velocity_pred = self(x_t, condition, x_mask, t)
        loss = masked_mse(velocity_pred, velocity_target, x_mask)
        endpoint_pred = (x_t + (1.0 - t_view) * velocity_pred) * x_mask
        if numeric_stability_check:
            endpoint_pred = make_tensor_stable(endpoint_pred)
        return endpoint_pred, loss

    def sample(
        self,
        content_latent: torch.Tensor,
        x_mask: torch.Tensor,
        steps: int = V32_CFM_STEPS,
        *,
        requires_grad: bool = False,
        numeric_stability_check: bool = False,
    ) -> torch.Tensor:
        if steps <= 0:
            raise ValueError(f"steps must be positive, got {steps}.")
        with torch.set_grad_enabled(requires_grad):
            condition = content_latent * x_mask
            if numeric_stability_check:
                condition = make_tensor_stable(condition)
            x = condition
            batch_size = x.shape[0]
            dt = 1.0 / steps
            for step in range(steps):
                t = torch.full(
                    (batch_size,),
                    step * dt,
                    device=x.device,
                    dtype=x.dtype,
                )
                velocity = self(x, condition, x_mask, t)
                x = (x + velocity * dt) * x_mask
                if numeric_stability_check:
                    x = make_tensor_stable(x)
            return x * x_mask
