import math

import torch
from torch import nn
from torch.nn import functional as functional


class WeightNormConv1d(nn.Module):
    def __init__(self, input_channels: int, output_channels: int) -> None:
        super().__init__()
        weight = torch.empty(output_channels, input_channels, 1)
        nn.init.kaiming_uniform_(weight, a=math.sqrt(5))
        self.weight_v = nn.Parameter(weight)
        self.weight_g = nn.Parameter(
            torch.linalg.vector_norm(weight, dim=(1, 2), keepdim=True)
        )
        self.bias = nn.Parameter(torch.empty(output_channels))
        bound = 1 / math.sqrt(input_channels)
        nn.init.uniform_(self.bias, -bound, bound)
        self.weight_precomputed = False

    def forward(self, inputs: torch.Tensor) -> torch.Tensor:
        if self.weight_precomputed:
            weight = self.weight_v
        else:
            norm = torch.linalg.vector_norm(self.weight_v, dim=(1, 2), keepdim=True)
            weight = self.weight_v * self.weight_g / norm
        return functional.conv1d(inputs, weight, self.bias)


class FactorizedVectorQuantize(nn.Module):
    def __init__(
        self,
        input_dim: int,
        codebook_size: int,
        codebook_dim: int,
        commitment: float,
        codebook_loss_weight: float,
        use_l2_normlize: bool,
    ) -> None:
        super().__init__()
        self.input_dim = input_dim
        self.codebook_size = codebook_size
        self.codebook_dim = codebook_dim
        self.commitment = commitment
        self.codebook_loss_weight = codebook_loss_weight
        self.use_l2_normlize = use_l2_normlize
        if input_dim != codebook_dim:
            self.in_project: nn.Module = WeightNormConv1d(input_dim, codebook_dim)
            self.out_project: nn.Module = WeightNormConv1d(codebook_dim, input_dim)
        else:
            self.in_project = nn.Identity()
            self.out_project = nn.Identity()
        self.codebook = nn.Embedding(codebook_size, codebook_dim)

    def decode_code(self, indices: torch.Tensor) -> torch.Tensor:
        return functional.embedding(indices, self.codebook.weight).transpose(1, 2)

    def decode_latents(
        self, latents: torch.Tensor
    ) -> tuple[torch.Tensor, torch.Tensor]:
        batch, dimensions, time = latents.shape
        encodings = latents.transpose(1, 2).reshape(batch * time, dimensions)
        codebook = self.codebook.weight
        if self.use_l2_normlize:
            encodings = functional.normalize(encodings)
            codebook = functional.normalize(codebook)
        distance = (
            encodings.square().sum(1, keepdim=True)
            - 2 * encodings @ codebook.t()
            + codebook.square().sum(1, keepdim=True).t()
        )
        indices = (-distance).max(1).indices.reshape(batch, time)
        return self.decode_code(indices), indices

    def forward(
        self, inputs: torch.Tensor
    ) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor, torch.Tensor, torch.Tensor]:
        projected = self.in_project(inputs)
        quantized, indices = self.decode_latents(projected)
        zero_loss = torch.zeros(inputs.shape[0], device=inputs.device)
        straight_through = projected + (quantized - projected).detach()
        return (
            self.out_project(straight_through),
            zero_loss,
            zero_loss,
            indices,
            projected,
        )

    def vq2emb(self, indices: torch.Tensor) -> torch.Tensor:
        return self.out_project(self.decode_code(indices))


class ResidualVQ(nn.Module):
    def __init__(
        self,
        input_dim: int,
        codebook_size: int,
        codebook_dim: int,
        num_quantizers: int = 1,
    ) -> None:
        super().__init__()
        self.input_dim = input_dim
        self.num_quantizers = num_quantizers
        self.codebook_size = codebook_size
        self.codebook_dim = codebook_dim
        self.quantizer_type = "fvq"
        self.quantizer_dropout = 0.0
        self.quantizers = nn.ModuleList(
            FactorizedVectorQuantize(
                input_dim,
                codebook_size,
                codebook_dim,
                commitment=0.15,
                codebook_loss_weight=1.0,
                use_l2_normlize=True,
            )
            for _ in range(num_quantizers)
        )

    def forward(
        self, inputs: torch.Tensor
    ) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor, torch.Tensor, torch.Tensor]:
        quantized = torch.zeros_like(inputs)
        residual = inputs
        indices: list[torch.Tensor] = []
        commitment_losses: list[torch.Tensor] = []
        codebook_losses: list[torch.Tensor] = []
        quantized_layers: list[torch.Tensor] = []
        for module in self.quantizers:
            if not isinstance(module, FactorizedVectorQuantize):
                raise TypeError("Semantic codec has an invalid quantizer")
            layer, commitment, codebook, layer_indices, _ = module(residual)
            quantized = quantized + layer
            residual = residual - layer
            indices.append(layer_indices)
            commitment_losses.append(commitment.mean())
            codebook_losses.append(codebook.mean())
            quantized_layers.append(layer)
        return (
            quantized,
            torch.stack(indices),
            torch.stack(commitment_losses),
            torch.stack(codebook_losses),
            torch.stack(quantized_layers),
        )

    def vq2emb(self, indices: torch.Tensor) -> torch.Tensor:
        quantized: torch.Tensor | None = None
        for index, module in enumerate(self.quantizers):
            if not isinstance(module, FactorizedVectorQuantize):
                raise TypeError("Semantic codec has an invalid quantizer")
            layer = module.vq2emb(indices[index])
            quantized = layer if quantized is None else quantized + layer
        if quantized is None:
            raise RuntimeError("Semantic codec has no quantizers")
        return quantized


class ConvNeXtBlock(nn.Module):
    def __init__(
        self,
        dim: int,
        intermediate_dim: int,
        layer_scale_init_value: float,
    ) -> None:
        super().__init__()
        self.dwconv = nn.Conv1d(dim, dim, kernel_size=7, padding=3, groups=dim)
        self.adanorm = False
        self.norm = nn.LayerNorm(dim, eps=1e-6)
        self.pwconv1 = nn.Linear(dim, intermediate_dim)
        self.act = nn.GELU()
        self.pwconv2 = nn.Linear(intermediate_dim, dim)
        self.gamma = nn.Parameter(layer_scale_init_value * torch.ones(dim))

    def forward(self, inputs: torch.Tensor) -> torch.Tensor:
        residual = inputs
        hidden = self.dwconv(inputs).transpose(1, 2)
        hidden = self.pwconv2(self.act(self.pwconv1(self.norm(hidden))))
        hidden = (self.gamma * hidden).transpose(1, 2)
        return residual + hidden


class VocosBackbone(nn.Module):
    def __init__(
        self,
        input_channels: int,
        dim: int,
        intermediate_dim: int,
        num_layers: int,
    ) -> None:
        super().__init__()
        self.input_channels = input_channels
        self.embed = nn.Conv1d(input_channels, dim, kernel_size=7, padding=3)
        self.adanorm = False
        self.norm = nn.LayerNorm(dim, eps=1e-6)
        layer_scale = 1 / num_layers
        self.convnext = nn.ModuleList(
            ConvNeXtBlock(dim, intermediate_dim, layer_scale)
            for _ in range(num_layers)
        )
        self.final_layer_norm = nn.LayerNorm(dim, eps=1e-6)
        self.apply(self.initialize_weights)

    @staticmethod
    def initialize_weights(module: nn.Module) -> None:
        if isinstance(module, nn.Conv1d | nn.Linear):
            nn.init.trunc_normal_(module.weight, std=0.02)
            if module.bias is not None:
                nn.init.constant_(module.bias, 0)

    def forward(self, inputs: torch.Tensor) -> torch.Tensor:
        hidden = self.norm(self.embed(inputs).transpose(1, 2)).transpose(1, 2)
        for module in self.convnext:
            hidden = module(hidden)
        return self.final_layer_norm(hidden.transpose(1, 2))


class RepCodec(nn.Module):
    def __init__(
        self,
        codebook_size: int,
        hidden_size: int,
        codebook_dim: int,
        vocos_dim: int,
        vocos_intermediate_dim: int,
        vocos_num_layers: int,
    ) -> None:
        super().__init__()
        self.codebook_size = codebook_size
        self.codebook_dim = codebook_dim
        self.hidden_size = hidden_size
        self.vocos_dim = vocos_dim
        self.vocos_intermediate_dim = vocos_intermediate_dim
        self.vocos_num_layers = vocos_num_layers
        self.num_quantizers = 1
        self.downsample_scale = 1
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
        self.apply(self.initialize_weights)

    @staticmethod
    def initialize_weights(module: nn.Module) -> None:
        if isinstance(module, nn.Conv1d | nn.Linear):
            nn.init.trunc_normal_(module.weight, std=0.02)
            if module.bias is not None:
                nn.init.constant_(module.bias, 0)

    def quantize(self, inputs: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
        encoded = self.encoder(inputs.transpose(1, 2)).transpose(1, 2)
        quantized, indices, _, _, _ = self.quantizer(encoded)
        if indices.shape[0] != 1:
            raise RuntimeError("IndexTTS 2 semantic codec requires one quantizer")
        return indices.squeeze(0), quantized.transpose(1, 2)
