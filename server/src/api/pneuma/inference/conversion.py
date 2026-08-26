import logging
from functools import lru_cache
from api.pneuma.inference.configs.config import ConfigData
from api.pneuma.inference.f0.type import PitchMethod
from dataclasses import dataclass
from pathlib import Path
import traceback
from collections.abc import Generator
from datetime import datetime
from time import perf_counter
from typing import Final, TypeAlias

import librosa

from api.pneuma.inference.utils.checkpoint_paths import DEFAULT_WEIGHT_ROOT, safetensors_json_pair
from api.pneuma.inference.utils.types import (
    AudioInput,
    ModelVersion,
    MonoAudioArray,
    OutputAudioArray,
    RawAudioArray,
    ResampledMonoAudioArray,
)

import numpy as np
from api.pneuma.inference.device import (
    get_device,
    is_cuda_out_of_memory_error,
    use_half_precision,
)
from api.pneuma.inference.utils.checkpoint import (
    convert_legacy_inference_checkpoint,
    load_inference_checkpoint,
)
from api.pneuma.inference.utils.types.version import hubert_output_layer
from api.pneuma.inference.utils.audio import normalize_audio
from api.pneuma.inference.models.content_models import content_model_for_version
from api.pneuma.inference.models.model_types import PneumaModel
from api.pneuma.inference.models.models import (
    build_synthesizer,
    prepare_decoder,
)
from api.pneuma.inference.pipeline import (
    InferenceTimings,
    PipelineConfig,
    convert_audio_iter,
    make_pipeline_config,
)


NO_MODEL_NAME: Final = "none"
logger = logging.getLogger("shrimply.pneuma")
DEFAULT_AUDIO_SPEED: Final = 1.0
MIN_AUDIO_SPEED: Final = 0.5
MAX_AUDIO_SPEED: Final = 2.0
VoiceConversionOutput: TypeAlias = tuple[
    list[list[str]], tuple[int, OutputAudioArray] | None
]
VoiceConversionIterator: TypeAlias = Generator[VoiceConversionOutput, None, None]


def _as_float32_array(value: np.ndarray) -> np.ndarray:
    array = np.asarray(value, dtype=np.float32)
    if array.dtype != np.float32:
        raise TypeError(f"Expected float32 array, got {array.dtype}")
    return array


def _normalize_audio_dtype(value: RawAudioArray) -> np.ndarray:
    array = np.asarray(value)
    if np.issubdtype(array.dtype, np.integer):
        if array.dtype == np.int16:
            scale = float(np.iinfo(np.int16).max + 1)
        elif array.dtype == np.int32:
            scale = float(np.iinfo(np.int32).max + 1)
        elif array.dtype == np.int64:
            scale = float(np.iinfo(np.int64).max + 1)
        elif array.dtype == np.uint8:
            scale = float(np.iinfo(np.uint8).max)
        elif array.dtype == np.uint16:
            scale = float(np.iinfo(np.uint16).max)
        elif array.dtype == np.uint32:
            scale = float(np.iinfo(np.uint32).max)
        elif array.dtype == np.uint64:
            scale = float(np.iinfo(np.uint64).max)
        else:
            raise TypeError(f"Unsupported integer PCM dtype: {array.dtype}")
        if scale <= 0:
            raise ValueError(f"Invalid integer PCM dtype: {array.dtype}")
        return _as_float32_array(_as_float32_array(array) / scale)
    if np.issubdtype(array.dtype, np.floating):
        return _as_float32_array(array)
    raise TypeError(f"Unsupported audio dtype: {array.dtype}")


def _ensure_mono_audio(value: np.ndarray) -> MonoAudioArray:
    if value.ndim == 1:
        return _as_float32_array(value).reshape(-1)
    if value.ndim == 2:
        if value.shape[1] == 1:
            return _as_float32_array(value[:, 0]).reshape(-1)
        return _as_float32_array(value.mean(axis=1))
    raise TypeError(
        f"Expected mono or stereo audio array with 1 or 2 dimensions, got shape {value.shape}"
    )


def _coerce_input_audio(value: RawAudioArray) -> MonoAudioArray:
    return _ensure_mono_audio(_normalize_audio_dtype(value))


def adjust_audio_speed(
    audio: MonoAudioArray,
    sample_rate: int,
    speed: float | int,
    *,
    maintain_pitch: bool,
) -> MonoAudioArray:
    speed = float(speed)
    if not np.isfinite(speed):
        raise ValueError(f"Audio speed must be finite, got {speed!r}.")
    if speed < MIN_AUDIO_SPEED or speed > MAX_AUDIO_SPEED:
        raise ValueError(
            f"Audio speed must be between {MIN_AUDIO_SPEED:.1f}x and "
            f"{MAX_AUDIO_SPEED:.1f}x, got {speed:.2f}x."
        )
    if np.isclose(speed, DEFAULT_AUDIO_SPEED):
        return audio
    if sample_rate <= 0:
        raise ValueError(f"Expected a positive sample rate, got {sample_rate}.")

    if maintain_pitch:
        return _as_float32_array(
            librosa.effects.time_stretch(audio, rate=speed)
        ).reshape(-1)

    adjusted_sample_rate = max(1, round(sample_rate / speed))
    return _as_float32_array(
        librosa.resample(
            audio,
            orig_sr=sample_rate,
            target_sr=adjusted_sample_rate,
        )
    ).reshape(-1)


def is_no_model_name(model_name: str) -> bool:
    return model_name.strip().lower() == NO_MODEL_NAME


def _clean_model_name(model_name: str) -> str:
    if not isinstance(model_name, str):
        raise TypeError(
            f"Model name must be a string, got {type(model_name).__name__}."
        )
    return model_name.strip()


def _clean_input_audio(input_audio: AudioInput) -> AudioInput:
    if input_audio is None:
        raise TypeError("Input audio is required.")
    if not isinstance(input_audio, tuple) or len(input_audio) != 2:
        raise TypeError(
            f"Input audio must be a sample-rate/audio tuple, got {type(input_audio).__name__}."
        )
    sample_rate, raw_audio = input_audio
    if not isinstance(sample_rate, int):
        raise TypeError(
            f"Input audio sample rate must be an integer, got {type(sample_rate).__name__}."
        )
    if not isinstance(raw_audio, np.ndarray):
        raise TypeError(
            f"Input audio samples must be a numpy array, got {type(raw_audio).__name__}."
        )
    return sample_rate, raw_audio


def _status_rows(
    status: str,
    message: str,
    timings: InferenceTimings | None = None,
) -> list[list[str]]:
    stats = timings or InferenceTimings()
    total = (
        stats.denoise
        + stats.speed_adjustment
        + stats.model_loading
        + stats.feature_extraction
        + stats.pitch_extraction
        + stats.synthesis
    )
    rows = [
        ["status", status],
        ["message", message],
        ["timestamp", datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]],
        ["denoise (ms)", f"{stats.denoise * 1000:.2f}"],
        ["speed (ms)", f"{stats.speed_adjustment * 1000:.2f}"],
        ["model (ms)", f"{stats.model_loading * 1000:.2f}"],
        ["feature (ms)", f"{stats.feature_extraction * 1000:.2f}"],
        ["pitch (ms)", f"{stats.pitch_extraction * 1000:.2f}"],
        ["synthesis (ms)", f"{stats.synthesis * 1000:.2f}"],
        ["total (ms)", f"{total * 1000:.2f}"],
    ]
    logger.info(
        "Built inference status rows: status=%s, rows=%d, keys=%s, denoise_ms=%.2f, speed_ms=%.2f, model_ms=%.2f, feature_ms=%.2f, pitch_ms=%.2f, synthesis_ms=%.2f, total_ms=%.2f",
        status,
        len(rows),
        [key for key, _ in rows],
        stats.denoise * 1000,
        stats.speed_adjustment * 1000,
        stats.model_loading * 1000,
        stats.feature_extraction * 1000,
        stats.pitch_extraction * 1000,
        stats.synthesis * 1000,
        total * 1000,
    )
    return rows


def _cuda_inference_oom_status_rows(
    *,
    stage: str,
    error: BaseException,
    timings: InferenceTimings | None = None,
) -> list[list[str]]:
    detail = str(error).splitlines()[0].strip()
    message = f"CUDA out of memory while {stage}."
    if detail:
        message = f"{message} {detail}"
    logger.warning(message)
    return _status_rows("error", message, timings)


def resample_audio(
    audio_array: MonoAudioArray,
    orig_sr: int,
    target_sr: int,
) -> ResampledMonoAudioArray:
    if audio_array.size < 10:  # A reasonable minimum length for resampling
        raise ValueError(
            f"Mono audio signal length ({audio_array.size}) is too small to resample from {orig_sr} to {target_sr}. "
            "Ensure the audio file contains actual sound data."
        )

    resampled_audio = librosa.resample(
        audio_array,
        orig_sr=orig_sr,
        target_sr=target_sr,
    )
    return _as_float32_array(resampled_audio).reshape(-1)


def _convert_without_model_iter(
    input_audio: AudioInput,
    timings: InferenceTimings,
    speed: float | int,
    maintain_pitch: bool,
) -> VoiceConversionIterator:
    try:
        original_sr, raw_audio = input_audio
        if original_sr <= 0:
            raise ValueError(f"Expected a positive sample rate, got {original_sr}.")

        audio = _coerce_input_audio(raw_audio)
        if audio.size == 0:
            raise ValueError("Audio is required.")
        start_time = perf_counter()
        audio = adjust_audio_speed(
            audio,
            original_sr,
            speed,
            maintain_pitch=maintain_pitch,
        )
        timings.speed_adjustment += perf_counter() - start_time

        yield _status_rows("running", "Input audio prepared.", timings), None
        logger.info(
            "Inference no-model conversion succeeded: output_sr=%d, output_shape=%s",
            original_sr,
            audio.shape,
        )
        yield (
            _status_rows(
                "success",
                "No model selected; returned prepared input audio.",
                timings,
            ),
            (original_sr, audio),
        )
    except Exception as error:
        if is_cuda_out_of_memory_error(error):
            yield (
                _cuda_inference_oom_status_rows(
                    stage="preparing input audio",
                    error=error,
                    timings=timings,
                ),
                None,
            )
            return
        info = traceback.format_exc()
        logger.warning(info)
        yield _status_rows("error", f"Failed with error:\n{info}"), None


@dataclass(frozen=True, slots=True)
class LoadedVoiceModel:
    net_g: PneumaModel
    target_sr: int
    version: ModelVersion


@lru_cache(maxsize=1)
def load_voice_model(
    model_path: Path,
    mtime_ns: int,
    size: int,
    is_half: bool,
) -> LoadedVoiceModel:
    logger.info(
        "Loading voice model from %s (mtime_ns=%d, size=%d)",
        model_path,
        mtime_ns,
        size,
    )
    cpt = load_inference_checkpoint(model_path)
    target_sr = cpt["sample_rate"]
    voice_version = cpt["version"]

    net_g = build_synthesizer(cpt["version"], is_half)
    net_g.load_state_dict(cpt["weight"], strict=True)
    if is_half:
        try:
            net_g = net_g.half()
        except Exception as e:
            net_g = net_g.float()
            logger.warning(
                "Could not convert model to half; keeping float32. Error: %s",
                e,
            )
    else:
        net_g = net_g.float()

    net_g = prepare_decoder(net_g)
    net_g.eval()
    net_g = net_g.to(get_device()).eval()
    return LoadedVoiceModel(
        net_g=net_g,
        target_sr=target_sr,
        version=voice_version,
    )


def resolve_voice_model_path(model_name: str, weight_root: Path) -> Path:
    weight_root = weight_root.resolve()
    model_path = (weight_root / model_name).resolve()
    if not model_path.is_relative_to(weight_root):
        raise ValueError("Voice model must be inside the configured model directory")
    pair = safetensors_json_pair(model_path)
    if model_path.suffix:
        if model_path == pair.safetensors:
            return model_path
        return convert_legacy_inference_checkpoint(model_path)

    if pair.safetensors.exists():
        return pair.safetensors
    if pair.legacy_pth.exists():
        return convert_legacy_inference_checkpoint(pair.legacy_pth)

    return model_path


def _convert_with_model_iter(
    input_audio: AudioInput,
    pitch_offset: int,
    f0_method: PitchMethod,
    model: LoadedVoiceModel,
    pipeline_config: PipelineConfig,
    timings: InferenceTimings,
    speed: float | int,
    maintain_pitch: bool,
) -> VoiceConversionIterator:
    try:
        original_sr, raw_audio = input_audio
        audio = _coerce_input_audio(raw_audio)
        start_time = perf_counter()
        audio = adjust_audio_speed(
            audio,
            original_sr,
            speed,
            maintain_pitch=maintain_pitch,
        )
        timings.speed_adjustment += perf_counter() - start_time
        if original_sr != 16000:
            audio = resample_audio(audio, original_sr, 16000)
        audio = normalize_audio(audio)
        yield _status_rows("running", "Input audio prepared.", timings), None

        yield _status_rows("running", "Loading content model.", timings), None
        start_time = perf_counter()
        content_model = content_model_for_version(model.version)
        timings.model_loading += perf_counter() - start_time
        yield _status_rows("running", "Content model ready.", timings), None

        audio_opt: OutputAudioArray | None = None
        for step in convert_audio_iter(
            content_model=content_model,
            net_g=model.net_g,
            audio=audio,
            config=pipeline_config,
            timings=timings,
            pitch_offset=pitch_offset,
            f0_method=f0_method,
            feature_output_layer=hubert_output_layer(model.version),
            voice_version=model.version,
        ):
            if step.output_audio is None:
                yield _status_rows("running", step.message, timings), None
            else:
                audio_opt = step.output_audio

        if audio_opt is None:
            raise RuntimeError("Conversion finished without output audio.")

        logger.info(
            "Inference conversion succeeded: output_sr=%d, output_shape=%s, model_ms=%.2f, feature_ms=%.2f, pitch_ms=%.2f, synthesis_ms=%.2f",
            model.target_sr,
            audio_opt.shape,
            timings.model_loading * 1000,
            timings.feature_extraction * 1000,
            timings.pitch_extraction * 1000,
            timings.synthesis * 1000,
        )
        yield (
            _status_rows("success", "Conversion completed.", timings),
            (
                model.target_sr,
                audio_opt,
            ),
        )
    except Exception as error:
        if is_cuda_out_of_memory_error(error):
            yield (
                _cuda_inference_oom_status_rows(
                    stage="converting audio",
                    error=error,
                    timings=timings,
                ),
                None,
            )
            return
        info = traceback.format_exc()
        logger.warning(info)
        yield _status_rows("error", f"Failed with error:\n{info}"), None


def convert_voice_iter(
    model_name: str,
    input_audio: AudioInput,
    pitch_offset: int,
    f0_method: PitchMethod,
    *,
    config: ConfigData,
    weight_root: Path = DEFAULT_WEIGHT_ROOT,
    speed: float | int = DEFAULT_AUDIO_SPEED,
    maintain_pitch: bool = True,
    denoise_time: float = 0.0,
) -> VoiceConversionIterator:
    model_name = _clean_model_name(model_name)
    input_audio = _clean_input_audio(input_audio)
    if model_name == "":
        logger.warning("Inference conversion rejected: model is missing")
        yield _status_rows("error", "Model is required."), None
        return

    logger.info(
        "Inference conversion starting: model=%s, pitch_offset=%d, f0_method=%s, speed=%s, maintain_pitch=%s",
        model_name,
        pitch_offset,
        f0_method,
        speed,
        maintain_pitch,
    )
    timings = InferenceTimings(denoise=denoise_time)
    if is_no_model_name(model_name):
        logger.info("Inference conversion using no-model passthrough")
        yield from _convert_without_model_iter(
            input_audio, timings, speed, maintain_pitch
        )
        return

    yield _status_rows("running", "Loading voice model.", timings), None
    start_time = perf_counter()
    try:
        model_path = resolve_voice_model_path(model_name, weight_root)
        stat = model_path.stat()
        model = load_voice_model(
            model_path,
            stat.st_mtime_ns,
            stat.st_size,
            use_half_precision(),
        )
        pipeline_config = make_pipeline_config(
            model.target_sr,
            x_pad=config.x_pad,
            x_query=config.x_query,
            x_center=config.x_center,
            x_max=config.x_max,
        )
    except Exception as error:
        if is_cuda_out_of_memory_error(error):
            timings.model_loading += perf_counter() - start_time
            yield (
                _cuda_inference_oom_status_rows(
                    stage="loading voice model",
                    error=error,
                    timings=timings,
                ),
                None,
            )
            return
        info = traceback.format_exc()
        logger.warning(info)
        yield _status_rows("error", f"Failed with error:\n{info}", timings), None
        return
    timings.model_loading += perf_counter() - start_time
    yield _status_rows("running", "Voice model ready.", timings), None
    yield from _convert_with_model_iter(
        input_audio,
        pitch_offset,
        f0_method,
        model,
        pipeline_config,
        timings,
        speed,
        maintain_pitch,
    )
