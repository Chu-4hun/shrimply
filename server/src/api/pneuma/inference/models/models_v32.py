import torch
from torch import nn

from api.pneuma.inference.configs.v32_config import get_v32_config
from api.pneuma.inference.models import utils
from api.pneuma.inference.models.attentions_v32 import V32Encoder
from api.pneuma.inference.models.bigvgan import _BigVGANNSFBase
from api.pneuma.inference.models.cfm_v32 import V32LatentCFM
from api.pneuma.inference.models.models import (
    LengthTensor,
)
from api.pneuma.inference.models.stability import make_tensor_stable
from api.pneuma.inference.utils.types.tensors import SSLFeatureTensor

V32_CHUNK_CONTEXT_FRAMES = 24
V32_LATENT_SCALE = 3.0
V32_CONTENT_BOTTLENECK_CHANNELS = 256
V32_F0_MIN = 50.0
V32_F0_MAX = 1100.0


def trim_inference_inputs(
    phone: SSLFeatureTensor,
    continuous_f0: torch.Tensor,
    phone_lengths: LengthTensor,
) -> tuple[SSLFeatureTensor, torch.Tensor, LengthTensor]:
    valid_length = min(
        phone.shape[1],
        continuous_f0.shape[1],
        int(phone_lengths.max().item()),
    )
    if valid_length <= 0:
        raise ValueError("V3.2 inference received an empty frame sequence.")
    phone = phone[:, :valid_length, :]
    continuous_f0 = continuous_f0[:, :valid_length]
    phone_lengths = phone_lengths.clamp(max=valid_length)
    return phone, continuous_f0, phone_lengths


class BigVGANNSFGenerator(_BigVGANNSFBase):
    pass


class F0Conditioner(nn.Module):
    def __init__(self, hidden_channels: int) -> None:
        super().__init__()
        self.proj = nn.Sequential(
            nn.Linear(2, hidden_channels),
            nn.SiLU(),
            nn.Linear(hidden_channels, hidden_channels),
        )

    def forward(self, continuous_f0: torch.Tensor) -> torch.Tensor:
        voiced = continuous_f0 > 0.0
        log_f0_min = torch.tensor(V32_F0_MIN, device=continuous_f0.device).log()
        log_f0_range = torch.tensor(
            V32_F0_MAX / V32_F0_MIN,
            device=continuous_f0.device,
        ).log()
        log_f0 = torch.where(
            voiced,
            torch.log(continuous_f0.float().clamp_min(1.0)),
            torch.zeros_like(continuous_f0, dtype=torch.float32),
        )
        normalized_log_f0 = (log_f0 - log_f0_min) / log_f0_range.clamp_min(1e-6)
        normalized_log_f0 = normalized_log_f0.mul(2.0).sub(1.0).clamp(-1.0, 1.0)
        normalized_log_f0 = torch.where(
            voiced,
            normalized_log_f0,
            torch.zeros_like(normalized_log_f0),
        )
        features = torch.stack(
            [normalized_log_f0, voiced.to(dtype=normalized_log_f0.dtype)],
            dim=-1,
        )
        target_dtype: torch.dtype = next(self.proj.parameters()).dtype
        return self.proj(features.to(dtype=target_dtype)).to(dtype=continuous_f0.dtype)


class TextEncoderV32(nn.Module):
    def __init__(
        self,
        in_channels: int,
        out_channels: int,
        hidden_channels: int,
        n_heads: int,
        n_layers: int,
        kernel_size: int,
    ) -> None:
        super().__init__()
        self.out_channels = out_channels
        self.hidden_channels = hidden_channels
        self.in_channels = in_channels
        self.emb_phone = nn.Sequential(
            nn.LayerNorm(in_channels),
            nn.Linear(in_channels, V32_CONTENT_BOTTLENECK_CHANNELS),
            nn.SiLU(),
            nn.Linear(V32_CONTENT_BOTTLENECK_CHANNELS, hidden_channels),
        )
        self.f0_conditioner = F0Conditioner(hidden_channels)
        self.activation = nn.SiLU()
        self.encoder = V32Encoder(
            hidden_channels,
            n_heads,
            n_layers,
            kernel_size,
        )
        self.proj = nn.Conv1d(hidden_channels, out_channels, 1)

    def forward(
        self,
        phone: SSLFeatureTensor,
        continuous_f0: torch.Tensor,
        lengths: LengthTensor,
        skip_head: torch.Tensor | None = None,
    ) -> tuple[torch.Tensor, torch.Tensor]:
        if phone.shape[-1] != self.in_channels:
            raise ValueError(
                f"Expected SSL feature dimension {self.in_channels}, "
                f"got {phone.shape[-1]}."
            )
        frame_count = min(phone.shape[1], continuous_f0.shape[1])
        if frame_count <= 0:
            raise ValueError("V3.2 text encoder received an empty frame sequence.")
        phone = phone[:, :frame_count, :]
        continuous_f0 = continuous_f0[:, :frame_count]
        lengths = lengths.clamp(max=frame_count)
        x_phone = self.emb_phone(phone)
        f0_conditioning = self.f0_conditioner(continuous_f0).to(dtype=x_phone.dtype)
        x = self.activation(x_phone + f0_conditioning)
        x = torch.transpose(x, 1, -1)
        x_mask = torch.unsqueeze(utils.sequence_mask(lengths, x.size(2)), 1).to(x.dtype)
        x = self.encoder(x * x_mask, x_mask)
        if skip_head is not None:
            assert isinstance(skip_head, torch.Tensor)
            head = int(skip_head.item())
            x = x[:, :, head:]
            x_mask = x_mask[:, :, head:]
        z = torch.tanh(self.proj(x)) * V32_LATENT_SCALE * x_mask
        return z, x_mask


class SynthesizerTrnBigVGANV32(nn.Module):
    ssl_dim = 1024

    def __init__(self, *, is_half: bool = False) -> None:
        super().__init__()

        config = get_v32_config()

        inter_channels = config.model.inter_channels
        hidden_channels = config.model.hidden_channels
        n_heads = config.model.n_heads
        n_layers = config.model.n_layers
        kernel_size = config.model.kernel_size
        resblock = config.model.resblock
        resblock_kernel_sizes = config.model.resblock_kernel_sizes
        resblock_dilation_sizes = config.model.resblock_dilation_sizes
        upsample_rates = config.model.upsample_rates
        upsample_initial_channel = config.model.upsample_initial_channel
        upsample_kernel_sizes = config.model.upsample_kernel_sizes
        if config.model.gin_channels is not None:
            raise ValueError("V3.2 is speakerless and requires gin_channels=None.")
        self.gin_channels = 0

        self.enc_p = TextEncoderV32(
            self.ssl_dim,
            inter_channels,
            hidden_channels,
            n_heads,
            n_layers,
            kernel_size,
        )
        self.dec = BigVGANNSFGenerator(
            inter_channels,
            resblock,
            resblock_kernel_sizes,
            resblock_dilation_sizes,
            upsample_rates,
            upsample_initial_channel,
            upsample_kernel_sizes,
            sr=48000,
            is_half=is_half,
        )
        self.cfm = V32LatentCFM(
            inter_channels,
            n_heads,
            kernel_size,
        )

    def remove_weight_norm(self) -> None:
        self.dec.remove_weight_norm()

    def infer(
        self,
        phone: SSLFeatureTensor,
        phone_lengths: LengthTensor,
        continuous_f0: torch.Tensor,
        skip_head: torch.Tensor | None = None,
        return_length: torch.Tensor | None = None,
        return_length2: torch.Tensor | None = None,
        numeric_stability_check: bool = False,
    ) -> tuple[torch.Tensor, torch.Tensor, tuple[torch.Tensor, torch.Tensor]]:
        phone, continuous_f0, phone_lengths = trim_inference_inputs(
            phone, continuous_f0, phone_lengths
        )
        z_content, x_mask = self.enc_p(phone, continuous_f0, phone_lengths)
        if skip_head is not None and return_length is not None:
            assert isinstance(skip_head, torch.Tensor)
            assert isinstance(return_length, torch.Tensor)
            frame_count = z_content.shape[-1]
            head = min(max(int(skip_head.item()), 0), frame_count)
            length = min(max(int(return_length.item()), 0), frame_count - head)
            if length <= 0:
                raise ValueError("V3.2 inference requested an empty chunk.")
            context_head = max(head - V32_CHUNK_CONTEXT_FRAMES, 0)
            context_tail = min(head + length + V32_CHUNK_CONTEXT_FRAMES, frame_count)
            dec_head = head - context_head
            z_content = z_content[:, :, context_head:context_tail]
            x_mask = x_mask[:, :, context_head:context_tail]
            continuous_f0 = continuous_f0[:, head : head + length]
        else:
            dec_head = 0
            length = z_content.shape[-1]
            continuous_f0 = continuous_f0[:, :length]
        z = self.cfm.sample(
            z_content,
            x_mask,
            numeric_stability_check=numeric_stability_check,
        )
        z = z[:, :, dec_head : dec_head + length]
        x_mask = x_mask[:, :, dec_head : dec_head + length]
        z_decode = z * x_mask
        if numeric_stability_check:
            z_decode = make_tensor_stable(z_decode)
        o = self.dec(
            z_decode,
            continuous_f0,
            n_res=return_length2,
        )
        return o, x_mask, (z, z_content)
