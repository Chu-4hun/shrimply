from dataclasses import dataclass
from torch import Tensor, nn

from .acoustic_diffusion import DiffusionTransformerConfig
from .acoustic_flow import ConditionalFlowMatching
from .length_regulator import InterpolateRegulator


@dataclass(frozen=True, slots=True)
class AcousticModelConfig:
    regulator_channels: int = 512
    regulator_input_channels: int = 1_024
    regulator_codebook_size: int = 2_048
    regulator_sampling_ratios: tuple[int, ...] = (1, 1, 1, 1)
    gpt_latent_dimension: int = 1_280
    first_projection_dimension: int = 256
    second_projection_dimension: int = 128
    acoustic_condition_dimension: int = 1_024


class AcousticModel(nn.Module):
    def __init__(self, config: AcousticModelConfig = AcousticModelConfig()) -> None:
        super().__init__()
        self.models = nn.ModuleDict(
            {
                "cfm": ConditionalFlowMatching(DiffusionTransformerConfig()),
                "length_regulator": InterpolateRegulator(
                    channels=config.regulator_channels,
                    sampling_ratios=config.regulator_sampling_ratios,
                    in_channels=config.regulator_input_channels,
                    codebook_size=config.regulator_codebook_size,
                ),
                "gpt_layer": nn.Sequential(
                    nn.Linear(
                        config.gpt_latent_dimension,
                        config.first_projection_dimension,
                    ),
                    nn.Linear(
                        config.first_projection_dimension,
                        config.second_projection_dimension,
                    ),
                    nn.Linear(
                        config.second_projection_dimension,
                        config.acoustic_condition_dimension,
                    ),
                ),
            }
        )

    def prepare(self) -> None:
        flow = self.models["cfm"]
        if not isinstance(flow, ConditionalFlowMatching):
            raise TypeError("Acoustic model has an invalid flow component")
        flow.estimator.prepare()

    def project_gpt_latent(self, latent: Tensor) -> Tensor:
        return self.models["gpt_layer"](latent)

    def regulate_length(
        self, embeddings: Tensor, lengths: Tensor
    ) -> Tensor:
        regulator = self.models["length_regulator"]
        if not isinstance(regulator, InterpolateRegulator):
            raise TypeError("Acoustic model has an invalid length regulator")
        return regulator(embeddings, lengths)[0]

    def generate_mel(
        self,
        condition: Tensor,
        lengths: Tensor,
        prompt: Tensor,
        style: Tensor,
    ) -> Tensor:
        flow = self.models["cfm"]
        if not isinstance(flow, ConditionalFlowMatching):
            raise TypeError("Acoustic model has an invalid flow component")
        return flow.generate(condition, lengths, prompt, style)
