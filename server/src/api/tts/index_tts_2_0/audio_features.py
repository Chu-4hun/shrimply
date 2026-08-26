from dataclasses import dataclass
from functools import cache
from io import BytesIO

import librosa
import soundfile
import torch
import torchaudio
from torch import Tensor
from torch.nn import functional as F


@dataclass(frozen=True, slots=True)
class Waveform:
    samples: Tensor
    sample_rate: int


def decode_audio(
    encoded: bytes,
    sample_rate: int = 24_000,
    maximum_seconds: int = 15,
) -> Waveform:
    with soundfile.SoundFile(BytesIO(encoded)) as audio:
        if audio.format != "WAV":
            raise ValueError("Reference audio must be WAV")
        source_rate = audio.samplerate
        decoded = audio.read(
            frames=source_rate * maximum_seconds,
            dtype="float32",
            always_2d=True,
        )
    samples = torch.from_numpy(decoded).mean(dim=1, keepdim=False).unsqueeze(0)
    if samples.numel() == 0:
        raise ValueError("Reference audio is empty")
    if not torch.isfinite(samples).all():
        raise ValueError("Reference audio contains non-finite samples")
    waveform = Waveform(samples, source_rate)
    return Waveform(resample(waveform, sample_rate), sample_rate)


def resample(waveform: Waveform, sample_rate: int) -> Tensor:
    if waveform.sample_rate == sample_rate:
        return waveform.samples
    return torchaudio.functional.resample(
        waveform.samples, waveform.sample_rate, sample_rate
    )


def speaker_features(waveform: Waveform) -> Tensor:
    audio = resample(waveform, 16_000)
    features = torchaudio.compliance.kaldi.fbank(
        audio,
        num_mel_bins=80,
        dither=0,
        sample_frequency=16_000,
    )
    return features - features.mean(dim=0, keepdim=True)


def mel_spectrogram(waveform: Waveform, device: torch.device) -> Tensor:
    sample_rate = 22_050
    fft_size = 1_024
    hop_size = 256
    mel_channels = 80
    audio = resample(waveform, sample_rate).to(device)
    mel_basis = _mel_basis(device)
    window = _hann_window(device)
    padding = (fft_size - hop_size) // 2
    padded = F.pad(audio.unsqueeze(1), (padding, padding), mode="reflect").squeeze(1)
    spectrum = torch.stft(
        padded,
        fft_size,
        hop_length=hop_size,
        win_length=fft_size,
        window=window,
        center=False,
        pad_mode="reflect",
        normalized=False,
        onesided=True,
        return_complex=True,
    )
    magnitude = torch.sqrt(spectrum.abs().square() + 1e-9)
    return torch.log(torch.clamp(mel_basis @ magnitude, min=1e-5))


@cache
def _mel_basis(device: torch.device) -> Tensor:
    mel_filter = librosa.filters.mel(
        sr=22_050,
        n_fft=1_024,
        n_mels=80,
        fmin=0,
        fmax=None,
    )
    return torch.from_numpy(mel_filter).float().to(device)


@cache
def _hann_window(device: torch.device) -> Tensor:
    return torch.hann_window(1_024, device=device)
