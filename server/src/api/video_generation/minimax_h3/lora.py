from __future__ import annotations

import hashlib
import os
import re
from pathlib import Path
from typing import TYPE_CHECKING, Protocol

import env

if TYPE_CHECKING:
    from diffusers import MiniMaxH3Transformer3DModel


class TensorKeyReader(Protocol):
    def keys(self) -> list[str]: ...


DEFAULT_LORA_WEIGHT_NAME = "minimax_h3_looping_sketch_anime_v1.safetensors"
SKETCH_LORA_ID = "Inner-Reflections/MiniMax-H3-Looping-Sketch-Anime"
SKETCH_LORA_REVISION = "9c88fbc800ea87d745137f1b637c08aa1a5e3bd6"
CONVERSION_VERSION = "2"
MUSUBI_KEY = re.compile(
    r"^lora_unet_blocks_(?P<block>\d+)_(?P<module>attn_qkv_proj|attn_out_proj|mlp_fc1|mlp_fc2)"
    r"\.(?P<part>alpha|lora_down\.weight|lora_up\.weight)$"
)
DIRECT_TARGETS = {
    "attn_out_proj": "attn.to_out.0",
    "mlp_fc1": "ff.net.0.proj",
    "mlp_fc2": "ff.net.2",
}


def resolve_lora(source: str, weight_name: str | None = None) -> tuple[Path, str]:
    """Resolve a local Musubi adapter or download one file from a Hub repository."""
    local = Path(source).expanduser()
    if local.is_file():
        return local.resolve(), "local"
    if local.exists():
        raise ValueError(f"LoRA source is not a file: {local}")

    from huggingface_hub import HfApi, hf_hub_download

    filename = weight_name or DEFAULT_LORA_WEIGHT_NAME
    revision = (
        SKETCH_LORA_REVISION
        if source == SKETCH_LORA_ID
        else HfApi().model_info(source).sha
    )
    if revision is None:
        raise ValueError(f"LoRA repository {source!r} has no immutable revision")
    path = hf_hub_download(source, filename=filename, revision=revision)
    return Path(path), revision


def _converted_path(source: Path, revision: str) -> Path:
    identity = (
        f"{source.resolve()}:{source.stat().st_size}:{source.stat().st_mtime_ns}:{revision}:{CONVERSION_VERSION}"
    )
    digest = hashlib.sha256(identity.encode()).hexdigest()[:16]
    return env.minimax_h3_lora_cache_root() / f"{source.stem}-{digest}.diffusers.safetensors"


def _source_groups(handle: TensorKeyReader) -> dict[tuple[int, str], dict[str, str]]:
    groups: dict[tuple[int, str], dict[str, str]] = {}
    unknown = []
    for key in handle.keys():
        match = MUSUBI_KEY.fullmatch(key)
        if match is None:
            unknown.append(key)
            continue
        group = (int(match["block"]), match["module"])
        groups.setdefault(group, {})[match["part"]] = key
    if unknown:
        raise ValueError(f"unsupported MiniMax H3 LoRA tensors: {unknown[:3]}")
    expected = {(block, module) for block in range(50) for module in (*DIRECT_TARGETS, "attn_qkv_proj")}
    if set(groups) != expected:
        missing = sorted(expected - set(groups))
        extra = sorted(set(groups) - expected)
        raise ValueError(
            f"LoRA does not cover the expected 50 H3 blocks (missing={missing[:3]}, extra={extra[:3]})"
        )
    for group, parts in groups.items():
        if set(parts) != {"alpha", "lora_down.weight", "lora_up.weight"}:
            raise ValueError(f"incomplete LoRA tensor group {group}: {sorted(parts)}")
    return groups


def convert_musubi_lora(source: Path, output: Path) -> Path:
    """Convert Musubi's fused H3 projections to Diffusers/PEFT adapter keys."""
    import torch
    from safetensors import safe_open
    from safetensors.torch import save_file

    converted: dict[str, torch.Tensor] = {}
    output_metadata = {
        "format": "pt",
        "source_format": "musubi-minimax-h3",
        "source_file": source.name,
        "conversion": "split fused qkv up projection; fold alpha/rank into lora_B",
        "conversion_version": CONVERSION_VERSION,
    }
    with safe_open(source, framework="pt", device="cpu") as handle:
        metadata = handle.metadata() or {}
        architecture = metadata.get("modelspec.architecture")
        if architecture not in {None, "MiniMax-H3/lora"}:
            raise ValueError(f"expected a MiniMax-H3 LoRA, found architecture {architecture!r}")
        if training_mode := metadata.get("ss_h3_training_mode"):
            output_metadata["training_mode"] = training_mode
        groups = _source_groups(handle)
        for (block, module), parts in sorted(groups.items()):
            down = handle.get_tensor(parts["lora_down.weight"])
            up = handle.get_tensor(parts["lora_up.weight"])
            alpha = float(handle.get_tensor(parts["alpha"]).item())
            if down.ndim != 2 or up.ndim != 2 or down.shape[0] != up.shape[1]:
                raise ValueError(f"invalid LoRA shapes for block {block} {module}: {down.shape}, {up.shape}")
            rank = down.shape[0]
            scaled_up = up * (alpha / rank)
            prefix = f"transformer.transformer_blocks.{block}."
            if module == "attn_qkv_proj":
                if up.shape[0] % 3:
                    raise ValueError(f"QKV up projection cannot split into thirds: {up.shape}")
                for target, chunk in zip(("attn.to_q", "attn.to_k", "attn.to_v"), scaled_up.chunk(3, dim=0)):
                    converted[f"{prefix}{target}.lora_A.weight"] = down.clone()
                    converted[f"{prefix}{target}.lora_B.weight"] = chunk.contiguous()
            else:
                target = DIRECT_TARGETS[module]
                converted[f"{prefix}{target}.lora_A.weight"] = down
                converted[f"{prefix}{target}.lora_B.weight"] = scaled_up

    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(output.name + ".tmp")
    save_file(
        converted,
        str(temporary),
        metadata=output_metadata,
    )
    os.replace(temporary, output)
    return output


def prepare_lora(source: str, weight_name: str | None = None) -> Path:
    resolved, revision = resolve_lora(source, weight_name)
    output = _converted_path(resolved, revision)
    if not output.is_file():
        print(f"Converting Musubi MiniMax H3 LoRA to {output}", flush=True)
        convert_musubi_lora(resolved, output)
    return output


def load_lora(
    transformer: MiniMaxH3Transformer3DModel,
    source: str,
    scale: float,
    weight_name: str | None = None,
    workflow: str | None = None,
) -> Path:
    if not 0 <= scale <= 4:
        raise ValueError("LoRA scale must be between 0 and 4")
    load_adapter = getattr(transformer, "load_lora_adapter", None)
    set_adapters = getattr(transformer, "set_adapters", None)
    if not callable(load_adapter) or not callable(set_adapters):
        raise TypeError("MiniMax H3 transformer does not support LoRA adapters")
    converted = prepare_lora(source, weight_name)
    from safetensors import safe_open

    with safe_open(converted, framework="pt", device="cpu") as handle:
        training_mode = (handle.metadata() or {}).get("training_mode")
    if workflow == "ref2va" and training_mode and not training_mode.startswith("ref2va"):
        raise ValueError(
            f"LoRA was trained for {training_mode}, which uses H3's transformer partition; "
            "it cannot be applied to transformer_ref for ref2va"
        )
    load_adapter(
        converted,
        prefix="transformer",
        adapter_name="minimax_h3_user",
        low_cpu_mem_usage=True,
    )
    set_adapters("minimax_h3_user", weights=scale)
    return converted
