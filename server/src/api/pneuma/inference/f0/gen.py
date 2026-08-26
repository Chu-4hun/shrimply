from api.pneuma.inference.f0.type import PitchMethod

from collections.abc import Callable
from functools import lru_cache
from math import log
from typing import (
    TYPE_CHECKING,
    Literal,
    overload,
)

import torch
from api.pneuma.inference.device import get_device

if TYPE_CHECKING:
    from api.pneuma.inference.vendor.f0.rmvpe.rmvpe import RMVPE
    from api.pneuma.inference.f0.swift import SwiftF0Torch
    from torchfcpe.models_infer import InferCFNaiveMelPE

AudioTensor = torch.Tensor
FrameTensor = torch.Tensor
SourceFrameTensor = torch.Tensor
TargetFrameTensor = torch.Tensor
PitchTensor = torch.Tensor
ContinuousF0Tensor = torch.Tensor
ManualF0Tensor = torch.Tensor
F0Function = Callable[
    [
        AudioTensor,
        int | None,
        int,
        PitchMethod,
        ManualF0Tensor | list[list[float]] | None,
    ],
    tuple[PitchTensor, ContinuousF0Tensor],
]


@overload
def load_f0_model(model_name: Literal["rmvpe"], *, is_half: bool) -> "RMVPE": ...


@overload
def load_f0_model(
    model_name: Literal["fcpe"], *, is_half: bool
) -> "InferCFNaiveMelPE": ...


@overload
def load_f0_model(
    model_name: Literal["swift-f0"], *, is_half: bool
) -> "SwiftF0Torch": ...


@lru_cache(maxsize=3)
def load_f0_model(
    model_name: Literal["rmvpe", "fcpe", "swift-f0"], *, is_half: bool
) -> "RMVPE | InferCFNaiveMelPE | SwiftF0Torch":
    device = get_device()
    match model_name:
        case "rmvpe":
            from api.pneuma.inference.vendor.f0.rmvpe.rmvpe import RMVPE

            return RMVPE(is_half=is_half)
        case "fcpe":
            from torchfcpe import spawn_bundled_infer_model

            return spawn_bundled_infer_model(str(device))
        case "swift-f0":
            from api.pneuma.inference.f0.swift import load_swift_f0_model

            return (
                load_swift_f0_model(fmin=50.0, fmax=1100.0)
                .to(device)
                .eval()
                .requires_grad_(False)
            )


def _linear_interpolate(
    x: torch.Tensor,
    xp: torch.Tensor,
    fp: torch.Tensor,
) -> torch.Tensor:
    if xp.numel() == 0:
        return torch.zeros_like(x, dtype=torch.float32)
    if xp.numel() == 1:
        return torch.zeros_like(x, dtype=torch.float32) + fp[0].to(dtype=torch.float32)
    x = x.to(device=xp.device, dtype=torch.float32)
    xp = xp.to(dtype=torch.float32)
    fp = fp.to(device=xp.device, dtype=torch.float32)
    indices = torch.searchsorted(xp.contiguous(), x.contiguous())
    right = indices.clamp(1, xp.numel() - 1)
    left = right - 1
    left_x = xp[left]
    right_x = xp[right]
    left_y = fp[left]
    right_y = fp[right]
    denom = torch.clamp(right_x - left_x, min=torch.finfo(torch.float32).eps)
    weight = (x - left_x) / denom
    interpolated = left_y + weight * (right_y - left_y)
    interpolated = torch.where(x <= xp[0], fp[0], interpolated)
    return torch.where(x >= xp[-1], fp[-1], interpolated)


def post_process(
    tf0: int,
    f0: FrameTensor,
    f0_up_key: int,
    manual_x_pad: int,
    f0_mel_min: float,
    f0_mel_max: float,
    manual_f0: ManualF0Tensor | None = None,
) -> tuple[PitchTensor, ContinuousF0Tensor]:
    f0 = f0.to(dtype=torch.float32) * pow(2, f0_up_key / 12)
    if manual_f0 is not None:
        manual_f0 = manual_f0.to(device=f0.device, dtype=torch.float32)
        delta_t = int(
            torch.round((manual_f0[:, 0].max() - manual_f0[:, 0].min()) * tf0 + 1)
            .clamp(min=0)
            .item()
        )
        if delta_t > 0:
            replace_f0 = _linear_interpolate(
                torch.arange(delta_t, device=f0.device, dtype=torch.float32),
                manual_f0[:, 0] * tf0,
                manual_f0[:, 1],
            )
            start = manual_x_pad * tf0
            end = min(start + delta_t, f0.numel())
            if start < end:
                f0[start:end] = replace_f0[: end - start]
    f0_mel = 1127 * torch.log1p(f0 / 700)
    voiced = f0_mel > 0
    scaled = (f0_mel - f0_mel_min) * 254 / (f0_mel_max - f0_mel_min) + 1
    f0_mel = torch.where(voiced, scaled, f0_mel)
    f0_mel = torch.clamp(f0_mel, min=1, max=255)
    f0_coarse = torch.round(f0_mel).to(dtype=torch.int32)
    if f0_coarse.numel() and (int(f0_coarse.min()) < 0 or int(f0_coarse.max()) > 255):
        raise ValueError("Coarse pitch indices must stay within embedding range 0..255")
    return f0_coarse, f0.to(dtype=torch.float32)


def _to_float32_tensor(
    value: ManualF0Tensor | list[list[float]],
    *,
    device: torch.device | str | None = None,
) -> torch.Tensor:
    if torch.is_tensor(value):
        return value.to(device=device, dtype=torch.float32)
    return torch.as_tensor(value, dtype=torch.float32, device=device)


def _interpolate_f0(
    f0: FrameTensor,
) -> FrameTensor:
    data = f0.to(dtype=torch.float32).reshape(-1)
    voiced = data > 0.0
    if not bool(voiced.any()):
        return torch.zeros_like(data)
    positions = torch.arange(data.numel(), device=data.device, dtype=torch.float32)
    return _linear_interpolate(positions, positions[voiced], data[voiced])


def _resize_f0(source: SourceFrameTensor, target_len: int) -> TargetFrameTensor:
    if target_len <= 0:
        return torch.empty(0, dtype=torch.float32, device=source.device)
    source = source.to(dtype=torch.float32).reshape(-1)
    if source.numel() == 0:
        return torch.zeros(target_len, dtype=torch.float32, device=source.device)
    source = torch.where(source < 0.001, torch.nan, source)
    source_positions = torch.arange(
        source.numel(), device=source.device, dtype=torch.float32
    )
    target_positions = (
        torch.arange(target_len, device=source.device, dtype=torch.float32)
        * source.numel()
        / target_len
    )
    return torch.nan_to_num(
        _linear_interpolate(target_positions, source_positions, source)
    )


def extract_f0(
    x: AudioTensor,
    p_len: int | None,
    f0_up_key: int,
    f0_method: PitchMethod,
    *,
    is_half: bool,
    x_pad: int,
    window: int,
    sr: int,
    manual_f0: ManualF0Tensor | None = None,
) -> tuple[PitchTensor, ContinuousF0Tensor]:
    if p_len is None:
        p_len = x.shape[0] // window
    f0_min = 50
    f0_max = 1100

    match f0_method:
        case "crepe":
            import torchcrepe

            f0, pd = torchcrepe.predict(
                x.unsqueeze(0),
                sr,
                window,
                f0_min,
                f0_max,
                batch_size=512,
                device=x.device,
                return_periodicity=True,
            )
            pd = torchcrepe.filter.median(pd, 3)
            f0 = torchcrepe.filter.mean(f0, 3)
            f0[pd < 0.1] = 0
            f0 = _interpolate_f0(_resize_f0(f0[0], p_len))
        case "rmvpe":
            rmvpe = load_f0_model("rmvpe", is_half=is_half)
            f0 = rmvpe.infer_from_audio(x, thred=0.03)
            f0 = _interpolate_f0(_resize_f0(f0, p_len))
        case "fcpe":
            fcpe = load_f0_model("fcpe", is_half=is_half)
            inferred = fcpe.infer(
                x.unsqueeze(0),
                sr=sr,
                decoder_mode="local_argmax",
                threshold=0.006,
            )
            if not isinstance(inferred, torch.Tensor):
                raise TypeError("FCPE returned invalid pitch values")
            f0 = inferred.squeeze()
            f0 = _interpolate_f0(_resize_f0(f0, p_len))
        case "swift-f0":
            swift_f0 = load_f0_model("swift-f0", is_half=is_half)
            f0 = swift_f0.infer_from_audio(x)
            f0 = _interpolate_f0(_resize_f0(f0, p_len))

    coarse_f0, continuous_f0 = post_process(
        sr // window,
        f0,
        f0_up_key,
        x_pad,
        1127 * log(1 + f0_min / 700),
        1127 * log(1 + f0_max / 700),
        manual_f0,
    )
    return (
        coarse_f0.to(dtype=torch.int32),
        continuous_f0.to(dtype=torch.float32),
    )


def get_f0_func(
    is_half: bool,
    x_pad: int,
    window: int = 160,
    sr: int = 16000,
) -> F0Function:
    def calculate(
        x: AudioTensor,
        p_len: int | None,
        f0_up_key: int,
        f0_method: PitchMethod,
        manual_f0: ManualF0Tensor | list[list[float]] | None = None,
    ) -> tuple[PitchTensor, ContinuousF0Tensor]:
        manual_f0_tensor = (
            None
            if manual_f0 is None
            else _to_float32_tensor(manual_f0, device=x.device)
        )
        return extract_f0(
            x,
            p_len,
            f0_up_key,
            f0_method,
            is_half=is_half,
            x_pad=x_pad,
            window=window,
            sr=sr,
            manual_f0=manual_f0_tensor,
        )

    return calculate
