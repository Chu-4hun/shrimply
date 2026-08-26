import torch
from torch import nn
from torch.nn import functional as F

from api.tts.index_tts_2_0.semantic_codec import ResidualVQ, VocosBackbone


class EnhancedCodec(nn.Module):
    def __init__(
        self,
        codebook_size: int = 8_192,
        hidden_size: int = 1_024,
        codebook_dim: int = 8,
        vocos_dim: int = 384,
        vocos_intermediate_dim: int = 2_048,
        vocos_num_layers: int = 12,
    ) -> None:
        super().__init__()
        self.down = nn.Conv1d(
            hidden_size, hidden_size, kernel_size=3, stride=2, padding=1
        )
        self.up = nn.Conv1d(
            hidden_size, hidden_size, kernel_size=3, stride=1, padding=1
        )
        self.encoder = nn.Sequential(
            VocosBackbone(
                hidden_size,
                vocos_dim,
                vocos_intermediate_dim,
                vocos_num_layers,
            ),
            nn.Linear(vocos_dim, hidden_size),
        )
        self.decoder = nn.Sequential(
            VocosBackbone(
                hidden_size,
                vocos_dim,
                vocos_intermediate_dim,
                vocos_num_layers,
            ),
            nn.Linear(vocos_dim, hidden_size),
        )
        self.quantizer = ResidualVQ(hidden_size, codebook_size, codebook_dim)

    def decode(self, codes: torch.Tensor) -> torch.Tensor:
        if codes.ndim == 2:
            codes = codes.unsqueeze(0)
        quantized = self.quantizer.vq2emb(codes)
        decoded = self.decoder(quantized).transpose(1, 2)
        decoded = F.interpolate(decoded, scale_factor=2, mode="nearest")
        return self.up(decoded).transpose(1, 2)
