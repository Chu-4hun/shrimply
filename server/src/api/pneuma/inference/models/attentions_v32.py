import torch
from torch import nn
from torch.nn import functional as F

from api.pneuma.inference.models.blocks import RMSNorm

V32_FFN_CHANNELS = 512
ROPE_BASE = 10000.0
LOCAL_CONV_KERNEL_SIZE = 7


class HeadRMSNorm(nn.Module):
    def __init__(self, n_heads: int, head_channels: int, eps: float = 1e-6) -> None:
        super().__init__()
        self.eps = eps
        self.weight = nn.Parameter(torch.ones(n_heads, head_channels))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        input_dtype = x.dtype
        x_float = x.float()
        variance = x_float.pow(2).mean(dim=-1, keepdim=True)
        x = x_float * torch.rsqrt(variance + self.eps)
        return (x * self.weight.unsqueeze(0).unsqueeze(2)).to(dtype=input_dtype)


class RotaryEmbedding(nn.Module):
    def __init__(self, head_channels: int, base: float = ROPE_BASE) -> None:
        super().__init__()
        if head_channels % 2 != 0:
            raise ValueError("RoPE requires an even head dimension.")
        inv_freq = 1.0 / (
            base
            ** (torch.arange(0, head_channels, 2, dtype=torch.float32) / head_channels)
        )
        self.register_buffer("inv_freq", inv_freq, persistent=False)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        positions = torch.arange(x.shape[-2], device=x.device, dtype=torch.float32)
        inv_freq = self._buffers["inv_freq"]
        if inv_freq is None:
            raise RuntimeError("Rotary frequencies are unavailable")
        inv_freq = inv_freq.to(device=x.device)
        angles = torch.outer(positions, inv_freq)
        cos = angles.cos().to(dtype=x.dtype).unsqueeze(0).unsqueeze(0)
        sin = angles.sin().to(dtype=x.dtype).unsqueeze(0).unsqueeze(0)
        x_even = x[..., 0::2]
        x_odd = x[..., 1::2]
        rotated = torch.stack(
            (x_even * cos - x_odd * sin, x_even * sin + x_odd * cos),
            dim=-1,
        )
        return rotated.flatten(-2)


class V32Attention(nn.Module):
    def __init__(self, channels: int, n_heads: int) -> None:
        super().__init__()
        if channels % n_heads != 0:
            raise ValueError("channels must be divisible by n_heads.")
        self.channels = channels
        self.n_heads = n_heads
        self.head_channels = channels // n_heads
        self.conv_q = nn.Conv1d(channels, channels, 1)
        self.conv_k = nn.Conv1d(channels, channels, 1)
        self.conv_v = nn.Conv1d(channels, channels, 1)
        self.conv_o = nn.Conv1d(channels, channels, 1)
        self.q_norm = HeadRMSNorm(n_heads, self.head_channels)
        self.k_norm = HeadRMSNorm(n_heads, self.head_channels)
        self.rope = RotaryEmbedding(self.head_channels)
        nn.init.xavier_uniform_(self.conv_q.weight)
        nn.init.xavier_uniform_(self.conv_k.weight)
        nn.init.xavier_uniform_(self.conv_v.weight)

    def forward(self, x: torch.Tensor, x_mask: torch.Tensor) -> torch.Tensor:
        batch_size, _channels, frame_count = x.shape
        q = self._project(self.conv_q(x))
        k = self._project(self.conv_k(x))
        v = self._project(self.conv_v(x))
        q = self.rope(self.q_norm(q))
        k = self.rope(self.k_norm(k))
        valid_key_mask = x_mask.squeeze(1).to(dtype=torch.bool)[:, None, None, :]
        output = F.scaled_dot_product_attention(
            q,
            k,
            v,
            attn_mask=valid_key_mask,
            dropout_p=0.0,
            is_causal=False,
        )
        output = (
            output.transpose(2, 3)
            .contiguous()
            .view(batch_size, self.channels, frame_count)
        )
        return self.conv_o(output) * x_mask

    def _project(self, x: torch.Tensor) -> torch.Tensor:
        batch_size, _channels, frame_count = x.shape
        return x.view(
            batch_size,
            self.n_heads,
            self.head_channels,
            frame_count,
        ).transpose(2, 3)


class V32SwiGLUFFN(nn.Module):
    def __init__(
        self,
        channels: int,
        kernel_size: int,
        ffn_channels: int = V32_FFN_CHANNELS,
    ) -> None:
        super().__init__()
        self.kernel_size = kernel_size
        self.conv_gate = nn.Conv1d(channels, ffn_channels, kernel_size)
        self.conv_up = nn.Conv1d(channels, ffn_channels, kernel_size)
        self.conv_down = nn.Conv1d(ffn_channels, channels, kernel_size)

    def forward(self, x: torch.Tensor, x_mask: torch.Tensor) -> torch.Tensor:
        padded = self._same_padding(x * x_mask)
        x = F.silu(self.conv_gate(padded)) * self.conv_up(padded)
        x = self.conv_down(self._same_padding(x))
        return x * x_mask

    def _same_padding(self, x: torch.Tensor) -> torch.Tensor:
        if self.kernel_size == 1:
            return x
        pad_left = (self.kernel_size - 1) // 2
        pad_right = self.kernel_size // 2
        return F.pad(x, (pad_left, pad_right, 0, 0, 0, 0))


class V32LocalConvBranch(nn.Module):
    def __init__(
        self, channels: int, kernel_size: int = LOCAL_CONV_KERNEL_SIZE
    ) -> None:
        super().__init__()
        if kernel_size % 2 != 1:
            raise ValueError("Local depthwise conv kernel size must be odd.")
        self.norm = RMSNorm(channels)
        self.depthwise = nn.Conv1d(
            channels,
            channels,
            kernel_size,
            padding=kernel_size // 2,
            groups=channels,
        )
        self.pointwise = nn.Conv1d(channels, channels, 1)
        self.layer_scale = nn.Parameter(torch.full((channels, 1), 1e-4))

    def forward(self, x: torch.Tensor, x_mask: torch.Tensor) -> torch.Tensor:
        y = self.norm(x)
        y = self.depthwise(y * x_mask)
        y = F.silu(y)
        y = self.pointwise(y)
        return y * self.layer_scale.unsqueeze(0) * x_mask


class V32EncoderLayer(nn.Module):
    def __init__(self, channels: int, n_heads: int, kernel_size: int) -> None:
        super().__init__()
        self.attn_norm = RMSNorm(channels)
        self.attn = V32Attention(channels, n_heads)
        self.local = V32LocalConvBranch(channels)
        self.ffn_norm = RMSNorm(channels)
        self.ffn = V32SwiGLUFFN(channels, kernel_size)

    def forward(self, x: torch.Tensor, x_mask: torch.Tensor) -> torch.Tensor:
        x = x + self.attn(self.attn_norm(x), x_mask)
        x = x + self.local(x, x_mask)
        x = x + self.ffn(self.ffn_norm(x), x_mask)
        return x * x_mask


class V32Encoder(nn.Module):
    def __init__(
        self,
        hidden_channels: int,
        n_heads: int,
        n_layers: int,
        kernel_size: int,
    ) -> None:
        super().__init__()
        self.layers = nn.ModuleList(
            [
                V32EncoderLayer(hidden_channels, n_heads, kernel_size)
                for _ in range(n_layers)
            ]
        )

    def forward(self, x: torch.Tensor, x_mask: torch.Tensor) -> torch.Tensor:
        x = x * x_mask
        for layer in self.layers:
            x = layer(x, x_mask)
        return x * x_mask
