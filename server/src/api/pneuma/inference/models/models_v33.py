import torch
from torch import nn

from api.pneuma.inference.configs.v33_config import get_v33_config
from api.pneuma.inference.models import utils
from api.pneuma.inference.models.attentions_v33 import V33Encoder
from api.pneuma.inference.models.bigvgan import _BigVGANNSFBase
from api.pneuma.inference.models.flows_v33 import V33ResidualCouplingBlock
from api.pneuma.inference.models.models import LengthTensor
from api.pneuma.inference.utils.types.tensors import SSLFeatureTensor

V33_CHUNK_CONTEXT_FRAMES = 24
V33_F0_MIN = 50.0
V33_F0_MAX = 1100.0


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
        raise ValueError("V3.3 inference received an empty frame sequence.")
    phone = phone[:, :valid_length, :]
    continuous_f0 = continuous_f0[:, :valid_length]
    phone_lengths = phone_lengths.clamp(max=valid_length)
    return phone, continuous_f0, phone_lengths


class BigVGANNSFGeneratorV33(_BigVGANNSFBase):
    pass


class F0ConditionerV33(nn.Module):
    def __init__(self, hidden_channels: int) -> None:
        super().__init__()
        self.proj = nn.Sequential(
            nn.Linear(2, hidden_channels),
            nn.SiLU(),
            nn.Linear(hidden_channels, hidden_channels),
        )

    def forward(self, continuous_f0: torch.Tensor) -> torch.Tensor:
        voiced = continuous_f0 > 0.0
        log_f0_min = torch.tensor(V33_F0_MIN, device=continuous_f0.device).log()
        log_f0_range = torch.tensor(
            V33_F0_MAX / V33_F0_MIN,
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


class TextEncoderV33(nn.Module):
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
        self.in_channels = in_channels
        self.out_channels = out_channels
        self.hidden_channels = hidden_channels
        self.emb_phone = nn.Sequential(
            nn.LayerNorm(in_channels),
            nn.Linear(in_channels, hidden_channels),
            nn.SiLU(),
            nn.Linear(hidden_channels, hidden_channels),
        )
        self.f0_conditioner = F0ConditionerV33(hidden_channels)
        self.activation = nn.SiLU()
        self.encoder = V33Encoder(
            hidden_channels,
            n_heads,
            n_layers,
            kernel_size,
        )
        self.proj = nn.Conv1d(hidden_channels, out_channels * 2, 1)

    def forward(
        self,
        phone: SSLFeatureTensor,
        continuous_f0: torch.Tensor,
        lengths: LengthTensor,
        skip_head: torch.Tensor | None = None,
    ) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
        if phone.shape[-1] != self.in_channels:
            raise ValueError(
                f"Expected SSL feature dimension {self.in_channels}, "
                f"got {phone.shape[-1]}."
            )
        frame_count = min(phone.shape[1], continuous_f0.shape[1])
        if frame_count <= 0:
            raise ValueError("V3.3 text encoder received an empty frame sequence.")
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
            head = int(skip_head.item())
            x = x[:, :, head:]
            x_mask = x_mask[:, :, head:]
        stats = self.proj(x) * x_mask
        m, logs = torch.split(stats, self.out_channels, dim=1)
        return m, logs, x_mask


class SynthesizerTrnBigVGANV33(nn.Module):
    ssl_dim = 1024
    infer_noise_scale = 0.66666

    def __init__(self, *, is_half: bool = False) -> None:
        super().__init__()
        config = get_v33_config()

        inter_channels = config.model.inter_channels
        hidden_channels = config.model.hidden_channels
        filter_channels = config.model.filter_channels
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
            raise ValueError("V3.3 is speakerless and requires gin_channels=None.")
        self.gin_channels = 0

        self.enc_p = TextEncoderV33(
            self.ssl_dim,
            inter_channels,
            hidden_channels,
            n_heads,
            n_layers,
            kernel_size,
        )
        self.dec = BigVGANNSFGeneratorV33(
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
        self.flow = V33ResidualCouplingBlock(
            inter_channels,
            filter_channels,
            5,
            3,
        )

    def remove_weight_norm(self) -> None:
        self.dec.remove_weight_norm()
        self.flow.remove_weight_norm()

    def _sample_prior(
        self, m_p: torch.Tensor, logs_p: torch.Tensor, x_mask: torch.Tensor
    ) -> torch.Tensor:
        if self.infer_noise_scale == 0:
            return m_p * x_mask
        noise = torch.exp(logs_p) * torch.randn_like(m_p) * self.infer_noise_scale
        return (m_p + noise) * x_mask

    def infer(
        self,
        phone: SSLFeatureTensor,
        phone_lengths: LengthTensor,
        continuous_f0: torch.Tensor,
        skip_head: torch.Tensor | None = None,
        return_length: torch.Tensor | None = None,
        return_length2: torch.Tensor | None = None,
    ) -> tuple[
        torch.Tensor,
        torch.Tensor,
        tuple[torch.Tensor, torch.Tensor, torch.Tensor, torch.Tensor],
    ]:
        phone, continuous_f0, phone_lengths = trim_inference_inputs(
            phone, continuous_f0, phone_lengths
        )
        if skip_head is not None and return_length is not None:
            head = min(max(int(skip_head.item()), 0), phone.shape[1])
            length = min(max(int(return_length.item()), 0), phone.shape[1] - head)
            if length <= 0:
                raise ValueError("V3.3 inference requested an empty chunk.")
            flow_head = torch.tensor(
                max(head - V33_CHUNK_CONTEXT_FRAMES, 0),
                device=skip_head.device,
                dtype=skip_head.dtype,
            )
            dec_head = head - int(flow_head.item())
            m_p, logs_p, x_mask = self.enc_p(
                phone,
                continuous_f0,
                phone_lengths,
                flow_head,
            )
            z_p = self._sample_prior(m_p, logs_p, x_mask)
            z = self.flow(z_p, x_mask, reverse=True)
            z = z[:, :, dec_head : dec_head + length]
            x_mask = x_mask[:, :, dec_head : dec_head + length]
            continuous_f0 = continuous_f0[:, head : head + length]
        else:
            m_p, logs_p, x_mask = self.enc_p(phone, continuous_f0, phone_lengths)
            z_p = self._sample_prior(m_p, logs_p, x_mask)
            z = self.flow(z_p, x_mask, reverse=True)
            continuous_f0 = continuous_f0[:, : z.shape[-1]]
        o = self.dec(z * x_mask, continuous_f0, n_res=return_length2)
        return o, x_mask, (z, z_p, m_p, logs_p)
