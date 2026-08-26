import logging
from dataclasses import dataclass
from time import perf_counter
from collections.abc import Iterator
from typing import Protocol, TypeAlias

import numpy as np
from api.pneuma.inference.device import empty_cache, get_device, use_half_precision
from api.pneuma.inference.f0.gen import PitchMethod, extract_f0
from api.pneuma.inference.utils.hubert import HubertModelWrapper
from api.pneuma.inference.utils.types import (
    ModelVersion,
    MonoAudioArray,
    OutputAudioArray,
    ResampledMonoAudioArray,
)
import torch
import torch.nn.functional as F

SAMPLE_RATE = 16000
WINDOW = 160
logger = logging.getLogger("shrimply.pneuma")

BatchedCoarsePitchTensor = torch.Tensor
BatchedContinuousF0Tensor = torch.Tensor
type SynthesizerInferenceResult = tuple[
    torch.Tensor,
    torch.Tensor,
    tuple[torch.Tensor, ...],
]


def _as_float32_array(value: np.ndarray) -> np.ndarray:
    array = np.asarray(value, dtype=np.float32)
    if array.dtype != np.float32:
        raise TypeError(f"Expected float32 array, got {array.dtype}")
    return array


class CoarsePitchInferenceSynthesizer(Protocol):
    def infer(
        self,
        phone: torch.Tensor,
        phone_lengths: torch.Tensor,
        pitch: torch.Tensor,
        nsff0: torch.Tensor,
    ) -> SynthesizerInferenceResult: ...


class ContinuousF0InferenceSynthesizer(Protocol):
    def infer(
        self,
        phone: torch.Tensor,
        phone_lengths: torch.Tensor,
        continuous_f0: torch.Tensor,
    ) -> SynthesizerInferenceResult: ...


InferenceSynthesizer: TypeAlias = (
    CoarsePitchInferenceSynthesizer | ContinuousF0InferenceSynthesizer
)


@dataclass(slots=True)
class InferenceTimings:
    denoise: float = 0.0
    speed_adjustment: float = 0.0
    model_loading: float = 0.0
    feature_extraction: float = 0.0
    pitch_extraction: float = 0.0
    synthesis: float = 0.0


@dataclass(frozen=True, slots=True)
class ConversionStep:
    message: str
    output_audio: OutputAudioArray | None = None


@dataclass(frozen=True, slots=True)
class PipelineConfig:
    is_half: bool
    x_pad: int
    t_pad: int
    t_pad_tgt: int
    t_pad2: int
    t_query: int
    t_center: int
    t_max: int


def make_pipeline_config(
    target_sr: int,
    *,
    x_pad: int,
    x_query: int,
    x_center: int,
    x_max: int,
) -> PipelineConfig:
    is_half = use_half_precision()
    t_pad = SAMPLE_RATE * x_pad
    return PipelineConfig(
        is_half=is_half,
        x_pad=x_pad,
        t_pad=t_pad,
        t_pad_tgt=target_sr * x_pad,
        t_pad2=t_pad * 2,
        t_query=SAMPLE_RATE * x_query,
        t_center=SAMPLE_RATE * x_center,
        t_max=SAMPLE_RATE * x_max,
    )


def find_split_points(
    audio_filtered: MonoAudioArray,
    config: PipelineConfig,
) -> list[int]:
    audio_pad = _as_float32_array(
        np.pad(audio_filtered, (WINDOW // 2, WINDOW // 2), mode="reflect")
    )
    if audio_pad.shape[0] <= config.t_max:
        return []

    audio_sum = np.zeros_like(audio_filtered)
    for i in range(WINDOW):
        audio_sum += np.abs(audio_pad[i : i - WINDOW])

    split_points = []
    for t in range(config.t_center, audio_filtered.shape[0], config.t_center):
        search_window = audio_sum[t - config.t_query : t + config.t_query]
        split_points.append(
            t - config.t_query + np.where(search_window == search_window.min())[0][0]
        )
    return split_points


def extract_pitch(
    audio_pad: MonoAudioArray,
    config: PipelineConfig,
    pitch_offset: int,
    f0_method: PitchMethod,
    timings: InferenceTimings,
) -> tuple[BatchedCoarsePitchTensor, BatchedContinuousF0Tensor]:
    device = get_device()
    start_time = perf_counter()
    p_len = audio_pad.shape[0] // WINDOW
    coarse_pitch, continuous_f0 = extract_f0(
        torch.as_tensor(audio_pad, device=device, dtype=torch.float32),
        p_len,
        pitch_offset,
        f0_method,
        is_half=config.is_half,
        x_pad=config.x_pad,
        window=WINDOW,
        sr=SAMPLE_RATE,
    )
    coarse_pitch = (
        coarse_pitch[:p_len].to(device=device, dtype=torch.int32).unsqueeze(0)
    )
    continuous_f0 = (
        continuous_f0[:p_len].to(device=device, dtype=torch.float32).unsqueeze(0)
    )
    timings.pitch_extraction += perf_counter() - start_time
    return coarse_pitch, continuous_f0


def synthesize_segment(
    content_model: HubertModelWrapper,
    net_g: InferenceSynthesizer,
    audio: MonoAudioArray,
    coarse_pitch: BatchedCoarsePitchTensor,
    continuous_f0: BatchedContinuousF0Tensor,
    config: PipelineConfig,
    timings: InferenceTimings,
    feature_output_layer: int,
    voice_version: ModelVersion,
) -> ResampledMonoAudioArray:
    device = get_device()
    feats = torch.from_numpy(audio)
    if config.is_half:
        try:
            feats = feats.half()
        except Exception as e:
            logger.warning(
                "Could not convert audio features to half; keeping float32. Error: %s",
                e,
            )
            feats = feats.float()
    else:
        feats = feats.float()
    assert feats.dim() == 1, feats.dim()
    feats = feats.view(1, -1)
    padding_mask = torch.BoolTensor(feats.shape).to(device).fill_(False)

    inputs = {
        "source": feats.to(device),
        "padding_mask": padding_mask,
        "output_layer": feature_output_layer,
    }
    feature_start_time = perf_counter()
    with torch.no_grad():
        logits = content_model.extract_features(**inputs)
        feats = logits[0]

    feats = F.interpolate(feats.permute(0, 2, 1), scale_factor=2).permute(0, 2, 1)
    synthesis_start_time = perf_counter()
    p_len = audio.shape[0] // WINDOW
    if feats.shape[1] < p_len:
        p_len = feats.shape[1]
        coarse_pitch = coarse_pitch[:, :p_len]
        continuous_f0 = continuous_f0[:, :p_len]

    p_len_tensor = torch.tensor([p_len], device=device, dtype=torch.int32)
    infer = getattr(net_g, "infer", None)
    if not callable(infer):
        raise TypeError("Synthesizer has no inference method")
    with torch.no_grad():
        if voice_version in {"v3.2", "v3.3"}:
            infer_result = infer(
                phone=feats,
                phone_lengths=p_len_tensor,
                continuous_f0=continuous_f0,
            )
        else:
            infer_result = infer(
                phone=feats,
                phone_lengths=p_len_tensor,
                pitch=coarse_pitch,
                nsff0=continuous_f0,
            )
        if (
            not isinstance(infer_result, tuple)
            or not infer_result
            or not isinstance(infer_result[0], torch.Tensor)
        ):
            raise TypeError("Synthesizer returned invalid audio")
        audio_segment = _as_float32_array(
            infer_result[0][0, 0].data.cpu().float().numpy(),
        )
    del feats, p_len_tensor, padding_mask
    empty_cache()
    synthesis_end_time = perf_counter()
    timings.feature_extraction += synthesis_start_time - feature_start_time
    timings.synthesis += synthesis_end_time - synthesis_start_time
    return audio_segment


def normalize_output(audio: OutputAudioArray) -> OutputAudioArray:
    audio_peak = float(np.abs(audio).max()) if audio.size else 0.0
    if audio_peak > 0.99:
        audio = audio / audio_peak * 0.99
    return _as_float32_array(audio)


def convert_audio_iter(
    content_model: HubertModelWrapper,
    net_g: InferenceSynthesizer,
    audio: MonoAudioArray,
    config: PipelineConfig,
    timings: InferenceTimings,
    pitch_offset: int,
    f0_method: PitchMethod,
    feature_output_layer: int,
    voice_version: ModelVersion,
) -> Iterator[ConversionStep]:
    yield ConversionStep("Preparing conversion")

    audio_filtered = _as_float32_array(audio)
    yield ConversionStep("Input audio prepared")

    split_points = find_split_points(audio_filtered, config)
    yield ConversionStep(f"Found {len(split_points)} split point(s)")

    audio_pad = _as_float32_array(
        np.pad(audio_filtered, (config.t_pad, config.t_pad), mode="reflect")
    )
    yield ConversionStep("Audio padded for conversion")

    coarse_pitch, continuous_f0 = extract_pitch(
        audio_pad,
        config,
        pitch_offset,
        f0_method,
        timings,
    )
    yield ConversionStep(f"Extracted F0 with {f0_method}")

    yield ConversionStep("Pitch tensors ready")

    start = 0
    last_split: int | None = None
    total_segments = len(split_points) + 1
    audio_segments: list[ResampledMonoAudioArray] = []
    for index, split_point in enumerate(split_points):
        split_point = split_point // WINDOW * WINDOW
        audio_segments.append(
            synthesize_segment(
                content_model=content_model,
                net_g=net_g,
                audio=audio_pad[start : split_point + config.t_pad2 + WINDOW],
                coarse_pitch=coarse_pitch[
                    :, start // WINDOW : (split_point + config.t_pad2) // WINDOW
                ],
                continuous_f0=continuous_f0[
                    :, start // WINDOW : (split_point + config.t_pad2) // WINDOW
                ],
                config=config,
                timings=timings,
                feature_output_layer=feature_output_layer,
                voice_version=voice_version,
            )[config.t_pad_tgt : -config.t_pad_tgt]
        )
        yield ConversionStep(f"Segment {index + 1}/{total_segments} converted")
        start = split_point
        last_split = split_point

    final_index = len(split_points)
    final_start = 0 if last_split is None else last_split
    audio_segments.append(
        synthesize_segment(
            content_model=content_model,
            net_g=net_g,
            audio=audio_pad[final_start:],
            coarse_pitch=coarse_pitch[:, final_start // WINDOW :],
            continuous_f0=continuous_f0[:, final_start // WINDOW :],
            config=config,
            timings=timings,
            feature_output_layer=feature_output_layer,
            voice_version=voice_version,
        )[config.t_pad_tgt : -config.t_pad_tgt]
    )
    yield ConversionStep(f"Segment {final_index + 1}/{total_segments} converted")

    audio_opt = _as_float32_array(np.concatenate(audio_segments))
    yield ConversionStep("Converted segments concatenated")

    audio_opt = normalize_output(audio_opt)
    yield ConversionStep("Output level normalized")

    del coarse_pitch, continuous_f0
    empty_cache()
    yield ConversionStep("Conversion complete.", audio_opt)
