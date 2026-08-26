import torch
from torch import Tensor, nn

from .acoustic_diffusion import (
    DiffusionTransformer,
    DiffusionTransformerConfig,
)


class ConditionalFlowMatching(nn.Module):
    def __init__(self, config: DiffusionTransformerConfig) -> None:
        super().__init__()
        self.estimator = DiffusionTransformer(config)
        self.in_channels = config.mel_channels

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
        if steps < 1:
            raise ValueError("Diffusion steps must be positive")
        batch, sequence_length = condition.shape[:2]
        sample = torch.randn(
            batch,
            self.in_channels,
            sequence_length,
            device=condition.device,
        ) * temperature
        times = torch.linspace(0, 1, steps + 1, device=condition.device)
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
