import re
from pathlib import Path
from typing import Callable, Literal, TypedDict

import torch
from pydantic import ConfigDict, TypeAdapter
from safetensors import safe_open
from safetensors.torch import load_file, save_file

from api.pneuma.inference.configs.v2_config import get_v2_config
from api.pneuma.inference.configs.v3_config import get_v3_config
from api.pneuma.inference.configs.v32_config import get_v32_config
from api.pneuma.inference.utils.checkpoint_paths import safetensors_json_pair
from api.pneuma.inference.utils.types import ModelVersion, WeightMap
from api.pneuma.inference.utils.types.checkpoint import SynthesizerConfigValue

type SampleRate = Literal[48000]
type LegacyScalar = None | bool | int | float | str
type LegacyValue = (
    LegacyScalar
    | torch.Tensor
    | list[LegacyValue]
    | tuple[LegacyValue, ...]
    | dict[str, LegacyValue]
)
type LegacyParser[T] = Callable[[LegacyValue], T]

LEGACY_VALUE_ADAPTER = TypeAdapter(
    LegacyValue, config=ConfigDict(arbitrary_types_allowed=True)
)
CONFIG_ADAPTER = TypeAdapter(list[SynthesizerConfigValue])
MODEL_VERSION_ADAPTER = TypeAdapter(ModelVersion)


class InferenceCheckpoint(TypedDict):
    weight: WeightMap
    sample_rate: SampleRate
    version: ModelVersion
    epoch: int


def _validate_finite_weights(weights: WeightMap, *, context: str) -> None:
    nonfinite_keys = [
        key
        for key, tensor in weights.items()
        if tensor.is_floating_point() and not torch.isfinite(tensor).all()
    ]
    if nonfinite_keys:
        preview = ", ".join(nonfinite_keys[:10])
        suffix = (
            ""
            if len(nonfinite_keys) <= 10
            else f", ... ({len(nonfinite_keys)} total)"
        )
        raise ValueError(f"{context} contains non-finite tensors: {preview}{suffix}")


def load_inference_checkpoint(path: Path) -> InferenceCheckpoint:
    if path != safetensors_json_pair(path).safetensors:
        raise ValueError(f"Inference loading only supports .safetensors files: {path}")
    with safe_open(path, framework="pt", device="cpu") as checkpoint_file:
        metadata = checkpoint_file.metadata()
    if metadata is None:
        raise ValueError(f"Inference checkpoint metadata is missing: {path}")
    sample_rate = int(metadata["sample_rate"])
    version = MODEL_VERSION_ADAPTER.validate_python(metadata["version"], strict=True)
    if sample_rate != 48_000:
        raise ValueError(f"Unsupported inference checkpoint sample rate: {sample_rate}")
    weights = load_file(path)
    _validate_finite_weights(weights, context=f"Inference checkpoint {path}")
    return {
        "weight": weights,
        "sample_rate": 48_000,
        "version": version,
        "epoch": int(metadata["epoch"]),
    }


def _parse_literal[T](
    value: LegacyValue, values: dict[LegacyScalar, T], field_name: str
) -> T:
    if not isinstance(value, (bool, int, float, str)):
        raise ValueError(
            f"Unsupported legacy inference checkpoint {field_name}: {value!r}"
        )
    try:
        return values[value]
    except KeyError:
        raise ValueError(
            f"Unsupported legacy inference checkpoint {field_name}: {value!r}"
        ) from None


def _parse_epoch(value: LegacyValue) -> int:
    if isinstance(value, bool):
        raise TypeError(f"Invalid legacy inference checkpoint epoch: {value!r}")
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        if value.isdecimal():
            return int(value)
        match = re.fullmatch(r"(?P<epoch>\d+)epoch", value)
        if match is not None:
            return int(match.group("epoch"))
    raise ValueError(f"Invalid legacy inference checkpoint epoch: {value!r}")


def _parse_config(value: LegacyValue) -> list[SynthesizerConfigValue]:
    if not isinstance(value, list):
        raise TypeError(
            "Legacy inference checkpoint config must be a list, "
            f"got {type(value).__name__}."
        )
    normalized = [
        tuple(tuple(item) if isinstance(item, list) else item for item in entry)
        if isinstance(entry, list) and any(isinstance(item, list) for item in entry)
        else tuple(entry)
        if isinstance(entry, list)
        else entry
        for entry in value
    ]
    return CONFIG_ADAPTER.validate_python(normalized, strict=True)


def _expected_legacy_config(
    version: ModelVersion,
) -> list[SynthesizerConfigValue]:
    if version == "v3.2":
        config = get_v32_config()
    elif version == "v3":
        config = get_v3_config()
    elif version == "v2":
        config = get_v2_config()
    else:
        raise ValueError("V3.3 has no legacy inference checkpoint format.")
    return [
        config.data.filter_length // 2 + 1,
        32,
        config.model.inter_channels,
        config.model.hidden_channels,
        config.model.filter_channels,
        config.model.n_heads,
        config.model.n_layers,
        config.model.kernel_size,
        config.model.p_dropout,
        config.model.resblock,
        tuple(config.model.resblock_kernel_sizes),
        tuple(tuple(item) for item in config.model.resblock_dilation_sizes),
        tuple(config.model.upsample_rates),
        config.model.upsample_initial_channel,
        tuple(config.model.upsample_kernel_sizes),
        config.model.spk_embed_dim,
        config.model.gin_channels,
        config.data.sampling_rate,
    ]


def convert_legacy_inference_checkpoint(path: Path) -> Path:
    pair = safetensors_json_pair(path)
    if pair.safetensors.exists():
        return pair.safetensors
    raw = LEGACY_VALUE_ADAPTER.validate_python(
        torch.load(path, map_location="cpu", weights_only=False), strict=True
    )
    if not isinstance(raw, dict):
        raise TypeError(f"Legacy inference checkpoint must be a dict: {path}")
    weights = raw.get("weight")
    if not isinstance(weights, dict):
        raise TypeError(f"Legacy inference checkpoint is missing weight map: {path}")
    if raw.get("f0") != 1:
        raise ValueError(f"Legacy inference checkpoint must be an f0 model: {path}")
    sample_rates: dict[LegacyScalar, SampleRate] = {
        "48k": 48_000,
        "48000": 48_000,
        48_000: 48_000,
    }
    sample_rate = _parse_literal(raw["sr"], sample_rates, "sample rate")
    versions: dict[LegacyScalar, ModelVersion] = {
        "v2": "v2",
        2: "v2",
        "2": "v2",
        "v3": "v3",
        3: "v3",
        "3": "v3",
        "v3.2": "v3.2",
        3.2: "v3.2",
        "3.2": "v3.2",
    }
    version = _parse_literal(
        raw["version"],
        versions,
        "version",
    )
    if _parse_config(raw["config"]) != _expected_legacy_config(version):
        raise ValueError(
            f"Legacy inference checkpoint config does not match {version}/{sample_rate}: {path}"
        )
    epoch = _parse_epoch(raw["info"])
    typed_weights: WeightMap = {}
    for key, value in weights.items():
        if not isinstance(value, torch.Tensor):
            raise TypeError(f"Legacy inference checkpoint weight {key!r} is not a tensor")
        typed_weights[key] = value
    _validate_finite_weights(typed_weights, context=f"Legacy checkpoint {path}")
    save_file(
        {key: tensor.detach().cpu().float() if tensor.is_floating_point() else tensor.detach().cpu() for key, tensor in typed_weights.items()},
        pair.safetensors,
        metadata={
            "experiment_name": pair.safetensors.stem,
            "sample_rate": str(sample_rate),
            "version": version,
            "epoch": str(epoch),
        },
    )
    return pair.safetensors
