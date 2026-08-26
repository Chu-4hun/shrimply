from pathlib import Path
from typing import BinaryIO

import librosa
import numpy as np
import soundfile as sf

from api.pneuma.resources import MAXIMUM_DECODED_AUDIO_BYTES
from api.pneuma.inference.utils.types import MonoAudioArray

type AudioInputFile = Path | BinaryIO


def get_audio_sample_rate(
    file: AudioInputFile, *, source_name: str | None = None
) -> int:
    display_name = source_name or str(file)
    try:
        with sf.SoundFile(file) as audio_file:
            sample_rate = int(audio_file.samplerate)
            if sample_rate <= 0:
                raise RuntimeError(
                    f"Audio file {display_name} is missing a sample rate."
                )
            return sample_rate
    except Exception as e:
        raise RuntimeError(
            f"Failed to read audio sample rate with SoundFile: {e}"
        ) from e


def _as_float32_array(value: np.ndarray) -> np.ndarray:
    array = np.asarray(value, dtype=np.float32)
    if array.dtype != np.float32:
        raise TypeError(f"Expected float32 array, got {array.dtype}")
    return array


def _ensure_mono_audio(value: np.ndarray) -> MonoAudioArray:
    array = _as_float32_array(value)
    if array.ndim == 1:
        return array.reshape(-1)
    if array.ndim == 2:
        if array.shape[1] == 1:
            return _as_float32_array(array[:, 0]).reshape(-1)
        return _as_float32_array(array.mean(axis=1)).reshape(-1)
    raise TypeError(f"Expected audio with 1 or 2 dimensions, got {array.shape}")


def _read_audio(file: AudioInputFile, display_name: str) -> tuple[int, MonoAudioArray]:
    with sf.SoundFile(file) as audio_file:
        input_sr = int(audio_file.samplerate)
        if input_sr <= 0:
            raise RuntimeError(f"Audio file {display_name} is missing a sample rate.")
        if audio_file.frames * audio_file.channels * 4 > MAXIMUM_DECODED_AUDIO_BYTES:
            raise RuntimeError(f"Decoded audio file {display_name} is too large.")
        audio = _ensure_mono_audio(audio_file.read(dtype="float32", always_2d=False))
        if audio.size == 0:
            raise RuntimeError(f"No decodable audio frames found in {display_name}.")
        return input_sr, audio


def load_audio_with_sr(
    file: AudioInputFile, sr: int | None = None, *, source_name: str | None = None
) -> tuple[int, MonoAudioArray]:
    display_name = source_name or str(file)
    try:
        input_sr, audio = _read_audio(file, display_name)
        output_sr = sr or input_sr
        if output_sr == input_sr:
            return output_sr, audio
        output_frames = (audio.size * output_sr + input_sr - 1) // input_sr
        if output_frames * 4 > MAXIMUM_DECODED_AUDIO_BYTES:
            raise RuntimeError(f"Resampled audio file {display_name} is too large.")
        resampled = librosa.resample(audio, orig_sr=input_sr, target_sr=output_sr)
        return output_sr, _as_float32_array(resampled).reshape(-1)
    except Exception as e:
        raise RuntimeError(f"Failed to load audio with SoundFile: {e}") from e


def load_audio(file: Path, sr: int) -> MonoAudioArray:
    _, audio = load_audio_with_sr(file, sr=sr)
    return audio


def normalize_audio(
    audio: MonoAudioArray,
    *,
    max_volume: float = 0.9,
    blend: float = 0.75,
) -> MonoAudioArray:
    audio = _as_float32_array(audio)
    peak = float(np.abs(audio).max()) if audio.size else 0.0
    if peak <= 0.0:
        return audio
    normalized = (audio / peak * (max_volume * blend)) + (1.0 - blend) * audio
    return _as_float32_array(normalized)
