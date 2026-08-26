import math
import logging
from collections.abc import Sequence
from typing import TYPE_CHECKING, Literal

import torch
from torch import nn

from api.pneuma.inference.models import attentions, utils
from api.pneuma.inference.models.bigvgan import SpeakerConditionedBigVGANNSFGenerator
from api.pneuma.inference.models.flows import Flip, ResidualCouplingLayer
from api.pneuma.inference.models.generators import (
    Generator,
    GeneratorNSF,
    SineGen,
    SourceModuleHnNSF,
)
from api.pneuma.inference.models.model_types import PneumaModel
from api.pneuma.inference.models.utils import remove_weight_norm
from api.pneuma.inference.utils.types import ModelVersion
from api.pneuma.inference.utils.types.tensors import SSLFeatureTensor

if TYPE_CHECKING:
    from api.pneuma.inference.configs.v2_config import V2Config
    from api.pneuma.inference.configs.v3_config import V3Config

type VCVersion = ModelVersion
type PitchTensor = torch.Tensor
type LengthTensor = torch.Tensor
type SpeakerTensor = torch.Tensor
logger = logging.getLogger("shrimply.pneuma")


class TextEncoder(nn.Module):
    emb_pitch: nn.Module
    lrelu: nn.Module

    def __init__(
        self,
        in_channels: int,
        out_channels: int,
        hidden_channels: int,
        filter_channels: int,
        n_heads: int,
        n_layers: int,
        kernel_size: int,
        p_dropout: float,
        pitch_proj: bool = False,
        activation: Literal["lrelu", "gelu"] = "lrelu",
        norm_type: Literal["layer", "rms"] = "layer",
        ffn_type: Literal["standard", "gated"] = "standard",
        ffn_activation: Literal["relu", "gelu", "silu"] = "relu",
        norm_position: Literal["post", "pre"] = "post",
    ) -> None:
        super(TextEncoder, self).__init__()
        self.out_channels = out_channels
        self.hidden_channels = hidden_channels
        self.filter_channels = filter_channels
        self.n_heads = n_heads
        self.n_layers = n_layers
        self.kernel_size = kernel_size
        self.p_dropout = p_dropout
        self.emb_phone = nn.Linear(in_channels, hidden_channels)
        if activation == "lrelu":
            self.lrelu = nn.LeakyReLU(0.1, inplace=True)
        elif activation == "gelu":
            self.lrelu = nn.GELU()
        else:
            raise ValueError(f"Unsupported activation: {activation}")
        if pitch_proj:
            self.emb_pitch = nn.Sequential(
                nn.Linear(1, 16),
                nn.GELU(),
                nn.Linear(16, 16),
            )
            self.concat_proj = nn.Linear(hidden_channels + 16, hidden_channels)
        else:
            self.emb_pitch = nn.Embedding(256, hidden_channels)  # pitch 256
        self.encoder = attentions.Encoder(
            hidden_channels,
            filter_channels,
            n_heads,
            n_layers,
            kernel_size,
            p_dropout,
            norm_type=norm_type,
            ffn_type=ffn_type,
            ffn_activation=ffn_activation,
            norm_position=norm_position,
        )
        self.proj = nn.Conv1d(hidden_channels, out_channels * 2, 1)

    def forward(
        self,
        phone: SSLFeatureTensor,
        pitch: PitchTensor,
        lengths: LengthTensor,
        skip_head: torch.Tensor | None = None,
    ) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
        if phone.shape[-1] != self.emb_phone.in_features:
            raise ValueError(
                f"Expected SSL feature dimension {self.emb_phone.in_features}, "
                f"got {phone.shape[-1]}."
            )
        if hasattr(self, "concat_proj"):
            p = pitch.unsqueeze(-1).float() / 255.0
            p_emb = self.emb_pitch(p)
            x_phone = self.emb_phone(phone)
            x = torch.cat([x_phone, p_emb], dim=-1)
            x = self.concat_proj(x)
        else:
            x = self.emb_phone(phone) + self.emb_pitch(pitch)
        x = x * math.sqrt(self.hidden_channels)  # [b, t, h]
        x = self.lrelu(x)
        x = torch.transpose(x, 1, -1)  # [b, h, t]
        x_mask = torch.unsqueeze(utils.sequence_mask(lengths, x.size(2)), 1).to(x.dtype)
        x = self.encoder(x * x_mask, x_mask)
        if skip_head is not None:
            assert isinstance(skip_head, torch.Tensor)
            head = int(skip_head.item())
            x = x[:, :, head:]
            x_mask = x_mask[:, :, head:]
        stats = self.proj(x) * x_mask
        m, logs = torch.split(stats, self.out_channels, dim=1)
        return m, logs, x_mask


class ResidualCouplingBlock(nn.Module):
    def __init__(
        self,
        channels: int,
        hidden_channels: int,
        kernel_size: int,
        dilation_rate: int,
        n_layers: int,
        n_flows: int = 4,
        gin_channels: int = 0,
    ) -> None:
        super(ResidualCouplingBlock, self).__init__()
        self.channels = channels
        self.hidden_channels = hidden_channels
        self.kernel_size = kernel_size
        self.dilation_rate = dilation_rate
        self.n_layers = n_layers
        self.n_flows = n_flows
        self.gin_channels = gin_channels

        self.flows = nn.ModuleList()
        for i in range(n_flows):
            self.flows.append(
                ResidualCouplingLayer(
                    channels,
                    hidden_channels,
                    kernel_size,
                    dilation_rate,
                    n_layers,
                    gin_channels=gin_channels,
                    mean_only=True,
                )
            )
            self.flows.append(Flip())

    def forward(
        self,
        x: torch.Tensor,
        x_mask: torch.Tensor,
        g: torch.Tensor | None = None,
        reverse: bool = False,
    ) -> torch.Tensor:
        flows = reversed(self.flows) if reverse else self.flows
        for flow in flows:
            if isinstance(flow, Flip):
                x, _ = flow(x)
            else:
                assert isinstance(flow, ResidualCouplingLayer)
                x, _ = flow(x, x_mask, g=g, reverse=reverse)
        return x

    def remove_weight_norm(self) -> None:
        for i in range(self.n_flows):
            flow = self.flows[i * 2]
            if not isinstance(flow, ResidualCouplingLayer):
                raise TypeError("Flow stack has an invalid residual coupling layer")
            flow.remove_weight_norm()


class SpeakerConditionedSynthesizerBase(nn.Module):
    ssl_dim = 768
    infer_noise_scale = 0.66666

    def _init_from_config(
        self,
        config: "V2Config | V3Config",
        *,
        sr: int,
        is_half: bool,
    ) -> None:
        inter_channels = config.model.inter_channels
        hidden_channels = config.model.hidden_channels
        filter_channels = config.model.filter_channels
        n_heads = config.model.n_heads
        n_layers = config.model.n_layers
        kernel_size = config.model.kernel_size
        p_dropout = config.model.p_dropout
        resblock = config.model.resblock
        resblock_kernel_sizes = config.model.resblock_kernel_sizes
        resblock_dilation_sizes = config.model.resblock_dilation_sizes
        upsample_rates = config.model.upsample_rates
        upsample_initial_channel = config.model.upsample_initial_channel
        upsample_kernel_sizes = config.model.upsample_kernel_sizes
        spk_embed_dim = config.model.spk_embed_dim
        gin_channels = config.model.gin_channels
        if gin_channels is None:
            raise ValueError("Speaker-conditioned models require gin_channels.")
        self.inter_channels = inter_channels
        self.hidden_channels = hidden_channels
        self.filter_channels = filter_channels
        self.n_heads = n_heads
        self.n_layers = n_layers
        self.kernel_size = kernel_size
        self.p_dropout = p_dropout
        self.resblock = resblock
        self.resblock_kernel_sizes = resblock_kernel_sizes
        self.resblock_dilation_sizes = resblock_dilation_sizes
        self.upsample_rates = upsample_rates
        self.upsample_initial_channel = upsample_initial_channel
        self.upsample_kernel_sizes = upsample_kernel_sizes
        self.gin_channels = gin_channels
        self.spk_embed_dim = spk_embed_dim
        self.enc_p = TextEncoder(
            self.ssl_dim,
            inter_channels,
            hidden_channels,
            filter_channels,
            n_heads,
            n_layers,
            kernel_size,
            p_dropout,
        )
        self.dec = self._build_decoder(
            inter_channels,
            resblock,
            resblock_kernel_sizes,
            resblock_dilation_sizes,
            upsample_rates,
            upsample_initial_channel,
            upsample_kernel_sizes,
            gin_channels=gin_channels,
            sr=sr,
            is_half=is_half,
        )
        self.flow = ResidualCouplingBlock(
            inter_channels, hidden_channels, 5, 1, 3, gin_channels=gin_channels
        )
        self.emb_g = nn.Embedding(self.spk_embed_dim, gin_channels)
        logger.debug(
            "gin_channels: "
            + str(gin_channels)
            + ", self.spk_embed_dim: "
            + str(self.spk_embed_dim)
        )

    def _speaker_conditioning(
        self, batch_size: int, device: torch.device
    ) -> torch.Tensor:
        speaker_id = torch.zeros((), dtype=torch.int32, device=device)
        g = self.emb_g(speaker_id).reshape(1, self.gin_channels, 1)
        return g.expand(batch_size, -1, -1)

    def _decode(
        self,
        z: torch.Tensor,
        pitchf: torch.Tensor,
        global_conditioning: torch.Tensor,
        return_length: torch.Tensor | None = None,
    ) -> torch.Tensor:
        if isinstance(self.dec, SpeakerConditionedBigVGANNSFGenerator):
            return self.dec(
                z,
                pitchf,
                global_conditioning=global_conditioning,
                n_res=return_length,
            )
        return self.dec(z, pitchf, g=global_conditioning, n_res=return_length)

    def _build_decoder(
        self,
        initial_channel: int,
        resblock: Literal["1", "2"],
        resblock_kernel_sizes: Sequence[int],
        resblock_dilation_sizes: Sequence[Sequence[int]],
        upsample_rates: Sequence[int],
        upsample_initial_channel: int,
        upsample_kernel_sizes: Sequence[int],
        gin_channels: int,
        sr: int,
        is_half: bool,
    ) -> (
        nn.Module | SpeakerConditionedBigVGANNSFGenerator
    ):
        raise NotImplementedError

    def remove_weight_norm(self) -> None:
        decoder_remove_weight_norm = getattr(self.dec, "remove_weight_norm", None)
        if not callable(decoder_remove_weight_norm):
            raise TypeError(
                f"{self.dec.__class__.__name__} does not support remove_weight_norm."
            )
        decoder_remove_weight_norm()
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
        pitch: PitchTensor,
        nsff0: torch.Tensor,
        skip_head: torch.Tensor | None = None,
        return_length: torch.Tensor | None = None,
        return_length2: torch.Tensor | None = None,
    ) -> tuple[
        torch.Tensor,
        torch.Tensor,
        tuple[torch.Tensor, torch.Tensor, torch.Tensor, torch.Tensor],
    ]:
        g = self._speaker_conditioning(phone.shape[0], phone.device)
        if skip_head is not None and return_length is not None:
            assert isinstance(skip_head, torch.Tensor)
            assert isinstance(return_length, torch.Tensor)
            head = int(skip_head.item())
            length = int(return_length.item())
            flow_head = torch.clamp(skip_head - 24, min=0)
            dec_head = head - int(flow_head.item())
            m_p, logs_p, x_mask = self.enc_p(phone, pitch, phone_lengths, flow_head)
            z_p = self._sample_prior(m_p, logs_p, x_mask)
            z = self.flow(z_p, x_mask, g=g, reverse=True)
            z = z[:, :, dec_head : dec_head + length]
            x_mask = x_mask[:, :, dec_head : dec_head + length]
            nsff0 = nsff0[:, head : head + length]
        else:
            m_p, logs_p, x_mask = self.enc_p(phone, pitch, phone_lengths)
            z_p = self._sample_prior(m_p, logs_p, x_mask)
            z = self.flow(z_p, x_mask, g=g, reverse=True)
        o = self._decode(z * x_mask, nsff0, g, return_length2)
        return o, x_mask, (z, z_p, m_p, logs_p)


__all__ = [
    "Generator",
    "GeneratorNSF",
    "SineGen",
    "SourceModuleHnNSF",
    "TextEncoder",
    "ResidualCouplingBlock",
    "VCVersion",
    "remove_weight_norm",
    "PneumaModel",
    "build_synthesizer",
    "prepare_decoder",
]


def build_synthesizer(version: ModelVersion, is_half: bool) -> PneumaModel:
    if version == "v3.3":
        from api.pneuma.inference.models.models_v33 import SynthesizerTrnBigVGANV33

        return SynthesizerTrnBigVGANV33(is_half=is_half)
    if version == "v3.2":
        from api.pneuma.inference.models.models_v32 import SynthesizerTrnBigVGANV32

        return SynthesizerTrnBigVGANV32(is_half=is_half)
    if version == "v3":
        from api.pneuma.inference.models.models_v3 import build_v3_synthesizer

        return build_v3_synthesizer(is_half)
    if version == "v2":
        from api.pneuma.inference.models.models_v2 import build_v2_synthesizer

        return build_v2_synthesizer(is_half)
    raise ValueError(f"Unsupported synthesizer version: {version}")


def prepare_decoder(net_g: PneumaModel) -> PneumaModel:
    logger.info(
        "Preparing %s generator for inference",
        (
            "V3 BigVGAN"
            if net_g.__class__.__name__
            in {
                "SynthesizerTrnBigVGANsid",
                "SynthesizerTrnBigVGANV32",
                "SynthesizerTrnBigVGANV33",
            }
            else "V2 NSF"
        ),
    )
    decoder_remove_weight_norm = getattr(net_g.dec, "remove_weight_norm", None)
    if not callable(decoder_remove_weight_norm):
        raise TypeError(
            f"{net_g.dec.__class__.__name__} does not support remove_weight_norm."
        )
    decoder_remove_weight_norm()
    return net_g
