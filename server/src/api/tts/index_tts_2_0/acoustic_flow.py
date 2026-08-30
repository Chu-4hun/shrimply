from collections import OrderedDict

import torch
from torch import Tensor, nn

from api.tts.index_cuda_graph import CudaGraphCache, replay_cuda_graph

from .acoustic_diffusion import (
    DiffusionTransformer,
    DiffusionTransformerConfig,
)


class ConditionalFlowMatching(nn.Module):
    def __init__(self, config: DiffusionTransformerConfig) -> None:
        super().__init__()
        self.estimator = DiffusionTransformer(config)
        self.in_channels = config.mel_channels
        self.use_cuda_graph = False
        self._cuda_graphs: CudaGraphCache = OrderedDict()

    @torch.inference_mode()
    def generate(
        self,
        condition: Tensor,
        lengths: Tensor,
        prompt: Tensor,
        style: Tensor,
        steps: int = 25,
        temperature: float = 1.0,
        guidance: float = 0.7,
    ) -> Tensor:
        if self.use_cuda_graph:
            if steps != 25 or temperature != 1.0 or guidance != 0.7:
                raise ValueError("IndexTTS CUDA Graph requires the production flow settings")
            sample = self._noise(condition, temperature)
            return replay_cuda_graph(
                self._cuda_graphs,
                self._integrate_default,
                (sample, condition, lengths, prompt, style),
            )
        return self._generate(
            condition,
            lengths,
            prompt,
            style,
            steps,
            temperature,
            guidance,
        )

    def _integrate_default(
        self,
        sample: Tensor,
        condition: Tensor,
        lengths: Tensor,
        prompt: Tensor,
        style: Tensor,
    ) -> Tensor:
        return self._integrate(sample, condition, lengths, prompt, style, 25, 0.7)

    def _generate(
        self,
        condition: Tensor,
        lengths: Tensor,
        prompt: Tensor,
        style: Tensor,
        steps: int,
        temperature: float,
        guidance: float,
    ) -> Tensor:
        if steps < 1:
            raise ValueError("Diffusion steps must be positive")
        sample = self._noise(condition, temperature)
        return self._integrate(
            sample,
            condition,
            lengths,
            prompt,
            style,
            steps,
            guidance,
        )

    def _noise(self, condition: Tensor, temperature: float) -> Tensor:
        return torch.randn(
            condition.shape[0],
            self.in_channels,
            condition.shape[1],
            device=condition.device,
            dtype=condition.dtype,
        ) * temperature

    def _integrate(
        self,
        sample: Tensor,
        condition: Tensor,
        lengths: Tensor,
        prompt: Tensor,
        style: Tensor,
        steps: int,
        guidance: float,
    ) -> Tensor:
        times = torch.linspace(
            0,
            1,
            steps + 1,
            device=condition.device,
            dtype=condition.dtype,
        )
        prompt_length = prompt.size(-1)
        prompt_values = torch.zeros_like(sample)
        prompt_values[..., :prompt_length] = prompt[..., :prompt_length]
        sample[..., :prompt_length] = 0
        time = times[0]
        for step in range(1, len(times)):
            delta = times[step] - times[step - 1]
            if guidance > 0:
                stacked_output = self.estimator(
                    torch.cat((sample, sample), dim=0),
                    torch.cat((prompt_values, torch.zeros_like(prompt_values)), dim=0),
                    lengths,
                    torch.stack((time, time)),
                    torch.cat((style, torch.zeros_like(style)), dim=0),
                    torch.cat((condition, torch.zeros_like(condition)), dim=0),
                )
                predicted, unconditioned = stacked_output.chunk(2, dim=0)
                derivative = (1 + guidance) * predicted - guidance * unconditioned
            else:
                derivative = self.estimator(
                    sample,
                    prompt_values,
                    lengths,
                    time.unsqueeze(0),
                    style,
                    condition,
                )
            sample = sample + delta * derivative
            time = time + delta
            sample[:, :, :prompt_length] = 0
        return sample
