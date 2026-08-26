import torch
from torch import nn
from torch.nn import functional as functional


def sequence_mask(lengths: torch.Tensor) -> torch.Tensor:
    positions = torch.arange(int(lengths.max()), device=lengths.device)
    return positions.unsqueeze(0) < lengths.unsqueeze(1)


class InterpolateRegulator(nn.Module):
    def __init__(
        self,
        channels: int,
        sampling_ratios: tuple[int, ...],
        in_channels: int,
        codebook_size: int,
        groups: int = 1,
    ) -> None:
        super().__init__()
        self.sampling_ratios = sampling_ratios
        self.interpolate = bool(sampling_ratios)
        layers: list[nn.Module] = []
        for _ in sampling_ratios:
            layers.extend(
                (
                    nn.Conv1d(channels, channels, 3, 1, 1),
                    nn.GroupNorm(groups, channels),
                    nn.Mish(),
                )
            )
        layers.append(nn.Conv1d(channels, channels, 1, 1))
        self.model = nn.Sequential(*layers)
        self.embedding = nn.Embedding(codebook_size, channels)
        self.is_discrete = False
        self.mask_token = nn.Parameter(torch.zeros(1, channels))
        self.n_codebooks = 1
        self.quantizer_dropout = 0.0
        self.f0_condition = False
        self.content_in_proj = nn.Linear(in_channels, channels)

    def forward(
        self,
        inputs: torch.Tensor,
        lengths: torch.Tensor,
        n_quantizers: int = 1,
        f0: torch.Tensor | None = None,
    ) -> tuple[torch.Tensor, torch.Tensor, None, None, None]:
        del n_quantizers, f0
        projected = self.content_in_proj(inputs)
        mask = sequence_mask(lengths).unsqueeze(-1)
        if not self.interpolate:
            raise RuntimeError("IndexTTS 2 requires an interpolating length regulator")
        interpolated = functional.interpolate(
            projected.transpose(1, 2).contiguous(),
            size=int(lengths.max()),
            mode="nearest",
        )
        output = self.model(interpolated).transpose(1, 2).contiguous()
        return output * mask, lengths, None, None, None
