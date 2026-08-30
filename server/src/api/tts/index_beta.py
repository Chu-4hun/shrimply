from collections import OrderedDict

import torch
from torch import nn

from .index_cuda_graph import CudaGraphCache, replay_cuda_graph
from .index_triton import require_triton, weight_norm
from .index_tts_2_0.acoustic_flow import ConditionalFlowMatching
from .index_tts_2_0.acoustic_diffusion import WeightNormLinear
from .index_tts_2_0.acoustic_transformer import AdaptiveLayerNorm
from .index_tts_2_0.acoustic_wavenet import WeightNormConv1d as FlowWeightNormConv1d
from .index_tts_2_0.model_loader import IndexModels
from .index_tts_2_0.semantic_codec import WeightNormConv1d as CodecWeightNormConv1d
from .index_tts_2_0.vocoder import (
    SnakeBeta,
    WeightNormConv1d,
    WeightNormConvTranspose1d,
)
from .index_tts_2_5.model_loader import IndexTts25Models


class CudaGraphModule(nn.Module):
    def __init__(self, module: nn.Module, pool) -> None:
        super().__init__()
        self.module = module
        self.pool = pool
        self.graphs: CudaGraphCache = OrderedDict()

    def forward(self, values: torch.Tensor) -> torch.Tensor:
        def run(inputs: torch.Tensor) -> torch.Tensor:
            output = self.module(inputs)
            if not isinstance(output, torch.Tensor):
                raise TypeError("IndexTTS GPT MLP returned a non-tensor output")
            return output

        return replay_cuda_graph(self.graphs, run, (values,), self.pool)


@torch.no_grad()
def prepare_index_beta(models: IndexModels | IndexTts25Models) -> None:
    if models.device.type != "cuda":
        raise RuntimeError("IndexTTS Beta requires a CUDA model")
    require_triton()
    adaptive_norms = 0
    snake_activations = 0
    normalized_layers = 0
    graph_regions = 0
    gpt_graphs = 0
    gpt_pool = torch.cuda.graph_pool_handle()
    for block in models.gpt.gpt.h:
        mlp = block.mlp
        if not isinstance(mlp, nn.Module):
            raise TypeError("IndexTTS GPT block MLP is not a module")
        block._modules["mlp"] = CudaGraphModule(mlp, gpt_pool)
        gpt_graphs += 1
    for module in (*models.acoustic.modules(), *models.semantic_codec.modules()):
        if isinstance(module, ConditionalFlowMatching):
            module.use_cuda_graph = True
            graph_regions += 1
        elif isinstance(module, AdaptiveLayerNorm):
            module.use_triton = True
            adaptive_norms += 1
        elif isinstance(
            module,
            WeightNormLinear | FlowWeightNormConv1d | CodecWeightNormConv1d,
        ):
            weight_norm(module.weight_v, module.weight_g, inplace=True)
            module.weight_precomputed = True
            normalized_layers += 1
    for module in models.vocoder.modules():
        if isinstance(module, SnakeBeta):
            module.use_triton = True
            snake_activations += 1
        elif isinstance(module, WeightNormConv1d | WeightNormConvTranspose1d):
            weight_norm(module.weight_v, module.weight_g, inplace=True)
            module.weight_precomputed = True
            normalized_layers += 1
    models.vocoder.use_cuda_graph = True
    graph_regions += 1
    if (
        not adaptive_norms
        or not snake_activations
        or not normalized_layers
        or gpt_graphs != 24
        or graph_regions != 2
    ):
        raise RuntimeError("IndexTTS Beta found no Triton-compatible model layers")
