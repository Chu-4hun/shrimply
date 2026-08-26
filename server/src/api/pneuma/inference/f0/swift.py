import logging
from api.pneuma.inference.device import get_device
from pathlib import Path
from typing import TYPE_CHECKING, Final
from urllib.request import urlretrieve

from safetensors.torch import load_model, save_file
import torch
import torch.nn as nn
import torch.nn.functional as F

from api.pneuma.inference.cache_paths import model_cache_dir

logger = logging.getLogger("shrimply.pneuma")

AudioTensor = torch.Tensor
BatchedAudioTensor = torch.Tensor
BatchedFrameTensor = torch.Tensor

if TYPE_CHECKING:
    from onnx import TensorProto

SWIFT_F0_MODEL_ROOT = model_cache_dir("swift-f0")
SWIFT_F0_MODEL_PATH = SWIFT_F0_MODEL_ROOT / "model.safetensors"
SWIFT_F0_ONNX_URL = (
    "https://github.com/lars76/swift-f0/raw/refs/heads/main/swift_f0/model.onnx"
)


class SwiftF0Torch(nn.Module):
    target_sample_rate: Final[int] = 16_000
    hop_length: Final[int] = 256
    frame_length: Final[int] = 1024
    stft_padding: Final[int] = (frame_length - hop_length) // 2
    min_audio_length: Final[int] = 256
    model_fmin: Final[float] = 46.875
    model_fmax: Final[float] = 2093.75
    pitch_bins: Final[int] = 200
    local_argmax_radius: Final[int] = 9

    def __init__(
        self,
        *,
        confidence_threshold: float = 0.9,
        fmin: float = 50.0,
        fmax: float = 1100.0,
    ) -> None:
        super().__init__()
        if not 0.0 <= confidence_threshold <= 1.0:
            raise ValueError("confidence_threshold must be between 0.0 and 1.0")
        if fmin < self.model_fmin or fmax > self.model_fmax or fmin > fmax:
            raise ValueError(
                f"swift-f0 frequency range must be within "
                f"[{self.model_fmin}, {self.model_fmax}] Hz"
            )
        self.confidence_threshold = confidence_threshold
        self.fmin = fmin
        self.fmax = fmax

        self.register_buffer(
            "_stft_window",
            torch.empty(self.frame_length, dtype=torch.float32),
        )
        self.register_buffer(
            "_pitch_bin_centers",
            torch.empty(self.pitch_bins, dtype=torch.float32),
        )
        self.conv_layers = nn.Sequential(
            nn.Conv2d(1, 8, kernel_size=5, padding=2),
            nn.ReLU(),
            nn.Conv2d(8, 16, kernel_size=5, padding=2),
            nn.ReLU(),
            nn.Conv2d(16, 32, kernel_size=5, padding=2),
            nn.ReLU(),
            nn.Conv2d(32, 64, kernel_size=5, padding=2),
            nn.ReLU(),
            nn.Conv2d(64, 1, kernel_size=5, padding=2),
            nn.ReLU(),
        )
        self.freq_projection = nn.Conv1d(132, self.pitch_bins, kernel_size=1)

    @property
    def stft_window(self) -> torch.Tensor:
        value = self._buffers["_stft_window"]
        if value is None:
            raise RuntimeError("swift-f0 STFT window is unavailable")
        return value

    @property
    def pitch_bin_centers(self) -> torch.Tensor:
        value = self._buffers["_pitch_bin_centers"]
        if value is None:
            raise RuntimeError("swift-f0 pitch bins are unavailable")
        return value

    def forward(
        self, audio: AudioTensor | BatchedAudioTensor
    ) -> tuple[BatchedFrameTensor, BatchedFrameTensor]:
        if audio.ndim == 1:
            audio = audio.unsqueeze(0)
        elif audio.ndim != 2:
            raise ValueError(f"swift-f0 expects 1D or 2D audio, got {audio.ndim}D")
        if audio.shape[-1] == 0:
            raise ValueError("swift-f0 input audio cannot be empty")

        audio = audio.to(device=self.stft_window.device, dtype=torch.float32)
        if audio.shape[-1] < self.min_audio_length:
            audio = F.pad(audio, (0, self.min_audio_length - audio.shape[-1]))
        audio = F.pad(audio, (self.stft_padding, self.stft_padding))

        stft = torch.stft(
            audio,
            n_fft=self.frame_length,
            hop_length=self.hop_length,
            win_length=self.frame_length,
            window=self.stft_window.to(device=audio.device),
            center=False,
            onesided=True,
            return_complex=True,
        )
        spectrogram = torch.log(stft.abs()[:, 3:135, :] + 1e-8)
        features = self.conv_layers(spectrogram.unsqueeze(1)).squeeze(1)
        logits = self.freq_projection(features).transpose(1, 2)
        probabilities = torch.softmax(logits, dim=-1)

        flat_probabilities = probabilities.reshape(-1, self.pitch_bins)
        argmax_bins = torch.argmax(flat_probabilities, dim=-1)
        bins = torch.arange(self.pitch_bins, device=audio.device)
        local_mask = (bins.unsqueeze(0) - argmax_bins.unsqueeze(1)).abs()
        local_mask = local_mask <= self.local_argmax_radius
        local_probabilities = flat_probabilities * local_mask.to(dtype=torch.float32)
        confidence = local_probabilities.sum(dim=-1)
        pitch = (
            local_probabilities
            / (confidence.unsqueeze(-1) + 1e-7)
            * self.pitch_bin_centers.to(device=audio.device)
        ).sum(dim=-1)

        shape = probabilities.shape[:2]
        return pitch.reshape(shape), confidence.reshape(shape)

    def infer_from_audio(self, audio: AudioTensor) -> torch.Tensor:
        pitch, confidence = self(audio)
        pitch = pitch.squeeze(0)
        confidence = confidence.squeeze(0)
        voiced = (
            (confidence > self.confidence_threshold)
            & (pitch >= self.fmin)
            & (pitch <= self.fmax)
        )
        return torch.where(voiced, pitch, torch.zeros_like(pitch))


def download_swift_f0_onnx_model(
    target_dir: Path,
) -> Path:
    model_path = target_dir / "model.onnx"
    logger.info("Downloading swift-f0 ONNX model to temporary path %s", model_path)
    downloaded_path, _ = urlretrieve(SWIFT_F0_ONNX_URL, model_path)
    return Path(downloaded_path)


def convert_swift_f0_onnx_to_safetensors(
    onnx_path: Path,
    safetensors_path: Path = SWIFT_F0_MODEL_PATH,
) -> Path:
    import onnx
    from onnx import TensorProto

    initializers: dict[str, TensorProto] = {
        initializer.name: initializer
        for initializer in onnx.load(onnx_path).graph.initializer
    }
    model = SwiftF0Torch()
    state_dict = model.state_dict()
    tensor_map = {
        "_stft_window": "onnx::STFT_171",
        "_pitch_bin_centers": "pitch_bin_centers",
        "conv_layers.0.weight": "onnx::Conv_152",
        "conv_layers.0.bias": "onnx::Conv_153",
        "conv_layers.2.weight": "onnx::Conv_155",
        "conv_layers.2.bias": "onnx::Conv_156",
        "conv_layers.4.weight": "onnx::Conv_158",
        "conv_layers.4.bias": "onnx::Conv_159",
        "conv_layers.6.weight": "onnx::Conv_161",
        "conv_layers.6.bias": "onnx::Conv_162",
        "conv_layers.8.weight": "onnx::Conv_164",
        "conv_layers.8.bias": "onnx::Conv_165",
        "freq_projection.weight": "freq_projection.weight",
        "freq_projection.bias": "freq_projection.bias",
    }
    for state_name, initializer_name in tensor_map.items():
        state_dict[state_name] = _initializer_tensor(initializers, initializer_name)

    safetensors_path.parent.mkdir(parents=True, exist_ok=True)
    save_file(state_dict, safetensors_path)
    onnx_path.unlink(missing_ok=True)
    return safetensors_path


def load_swift_f0_model(
    model_path: Path = SWIFT_F0_MODEL_PATH,
    *,
    confidence_threshold: float = 0.9,
    fmin: float = 50.0,
    fmax: float = 1100.0,
) -> SwiftF0Torch:
    from api.pneuma.inference.model_assets import resolve_safetensors_asset

    resolved_path = resolve_safetensors_asset(
        safetensors_path=model_path,
        convert_legacy=convert_swift_f0_onnx_to_safetensors,
        asset_name="swift-f0",
        download_legacy=download_swift_f0_onnx_model,
    )
    model = SwiftF0Torch(
        confidence_threshold=confidence_threshold,
        fmin=fmin,
        fmax=fmax,
    )
    # model = model.to(get_device())
    load_model(model, resolved_path, device=str(get_device()))
    return model


def _initializer_tensor(
    initializers: dict[str, "TensorProto"], name: str
) -> torch.Tensor:
    from onnx import numpy_helper

    array = numpy_helper.to_array(initializers[name])
    return torch.from_numpy(array.copy()).to(dtype=torch.float32)
