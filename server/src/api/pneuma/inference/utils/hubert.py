import logging
from functools import lru_cache
from pathlib import Path
from typing import Literal

import torch
import torch.nn as nn
from huggingface_hub import hf_hub_download
from safetensors.torch import load_model, save_file

from api.pneuma.inference.device import get_device, use_half_precision
from api.pneuma.inference.utils.legacy_rvc import LEGACY_RVC_REPO_ID


def _disable_transformers_torchvision_imports() -> None:
    import transformers.utils as transformers_utils
    import transformers.utils.import_utils as import_utils

    def unavailable() -> bool:
        return False

    for availability_fn in (
        import_utils.is_torchvision_available,
        import_utils.is_torchvision_v2_available,
    ):
        cache_clear = getattr(availability_fn, "cache_clear", None)
        if callable(cache_clear):
            cache_clear()

    setattr(import_utils, "is_torchvision_available", unavailable)
    setattr(import_utils, "is_torchvision_v2_available", unavailable)
    setattr(transformers_utils, "is_torchvision_available", unavailable)
    setattr(transformers_utils, "is_torchvision_v2_available", unavailable)


_disable_transformers_torchvision_imports()

from transformers import HubertConfig, HubertForCTC, HubertModel
from api.pneuma.inference.cache_paths import model_cache_dir

logger = logging.getLogger("shrimply.pneuma")

type SSLDim = Literal[768, 1024]

HUBERT_MODEL_DIR = model_cache_dir("hubert")
HUBERT_BASE_MODEL_PATH = HUBERT_MODEL_DIR / "hubert_base.pt"
HUBERT_LARGE_MODEL_ID = "facebook/hubert-large-ls960-ft"
HUBERT_LARGE_PRUNED_MODEL_PATH = HUBERT_MODEL_DIR / "hubert_large_layer12.safetensors"
HUBERT_LARGE_PRUNED_MODEL_DIR = HUBERT_MODEL_DIR
HUBERT_BASE_FEATURE_DIM: SSLDim = 768
HUBERT_LARGE_FEATURE_DIM: SSLDim = 1024
HUBERT_OUTPUT_LAYERS = frozenset((9, 12))
HUBERT_WEIGHT_PARAM_NAMES = frozenset(("weight", "bias"))
HUBERT_ALLOWED_UNEXPECTED_KEYS = frozenset(
    ("encoder.layer_norm.bias", "encoder.layer_norm.weight")
)


class HubertModelWrapper(nn.Module):
    def __init__(
        self,
        hf_model: HubertModel,
        feature_dim: SSLDim,
        default_output_layer: int,
    ) -> None:
        super().__init__()
        self.model = hf_model
        self.feature_dim = feature_dim
        self.default_output_layer = default_output_layer

    def forward(
        self,
        source: torch.Tensor,
        padding_mask: torch.Tensor | None = None,
    ) -> torch.Tensor:
        features, _ = self.extract_features(
            source, padding_mask, output_layer=self.default_output_layer
        )
        return features

    def _validate_output_layer(self, output_layer: int) -> None:
        num_hidden_layers = self.model.config.num_hidden_layers
        if output_layer < 0 or output_layer > num_hidden_layers:
            raise ValueError(
                "Requested HuBERT output_layer={} but this model only has "
                "hidden-state indices 0..{}.".format(output_layer, num_hidden_layers)
            )

    def extract_features(
        self,
        source: torch.Tensor,
        padding_mask: torch.Tensor | None = None,
        output_layer: int = 12,
    ) -> tuple[torch.Tensor, torch.Tensor | None]:
        self._validate_output_layer(output_layer)

        # fairseq padding_mask is True for padding. Transformers attention_mask is 1 for NOT padding.
        attention_mask = None
        if padding_mask is not None:
            attention_mask = ~padding_mask

        outputs = self.model(
            input_values=source,
            attention_mask=attention_mask,
            output_hidden_states=True,
            return_dict=True,
        )

        # In fairseq, output features are shape (B, T, C). Transformers gives (B, T, C)
        # Hidden states: 0 is embedding, 1 to 12 are the transformer layers.
        # So output_layer=12 corresponds to hidden_states[12]
        feats = outputs.hidden_states[output_layer]
        if feats.shape[-1] != self.feature_dim:
            raise ValueError(
                f"Expected HuBERT feature dimension {self.feature_dim}, "
                f"got {feats.shape[-1]}."
            )

        # fairseq returns (features, padding_mask) or a similar tuple in legacy V2.
        # Let's match the original return type of (feature, padding_mask)
        return feats, padding_mask

    def infer(
        self,
        source: torch.Tensor,
        padding_mask: torch.Tensor | None,
        output_layer: torch.Tensor | int,
    ) -> torch.Tensor:
        if isinstance(output_layer, torch.Tensor):
            output_layer_id = int(output_layer.item())
        else:
            output_layer_id = output_layer

        if output_layer_id not in HUBERT_OUTPUT_LAYERS:
            raise ValueError(
                f"Only HuBERT output_layer=9 or 12 is supported. Got {output_layer_id}"
            )
        self._validate_output_layer(output_layer_id)

        logits, _ = self.extract_features(
            source=source, padding_mask=padding_mask, output_layer=output_layer_id
        )
        return logits


def convert_fairseq_to_hf(
    model_path: Path,
    safetensors_path: Path,
    save_dtype: torch.dtype | None = None,
) -> None:
    import sys
    import types

    if "fairseq" not in sys.modules:
        sys.modules["fairseq"] = types.ModuleType("fairseq")
        sys.modules["fairseq.data"] = types.ModuleType("fairseq.data")
        sys.modules["fairseq.data.dictionary"] = types.ModuleType(
            "fairseq.data.dictionary"
        )

        class DummyDict:
            pass

        setattr(sys.modules["fairseq.data.dictionary"], "Dictionary", DummyDict)

    ckpt = torch.load(model_path, weights_only=False)
    fairseq_dict = ckpt["model"]

    hf_config = HubertConfig()
    hf_model = HubertModel(hf_config)

    mapping = {
        "post_extract_proj": "feature_projection.projection",
        "encoder.pos_conv.0": "encoder.pos_conv_embed.conv",
        "self_attn.k_proj": "encoder.layers.*.attention.k_proj",
        "self_attn.v_proj": "encoder.layers.*.attention.v_proj",
        "self_attn.q_proj": "encoder.layers.*.attention.q_proj",
        "self_attn.out_proj": "encoder.layers.*.attention.out_proj",
        "self_attn_layer_norm": "encoder.layers.*.layer_norm",
        "fc1": "encoder.layers.*.feed_forward.intermediate_dense",
        "fc2": "encoder.layers.*.feed_forward.output_dense",
        "final_layer_norm": "encoder.layers.*.final_layer_norm",
        "encoder.layer_norm": "encoder.layer_norm",
        "layer_norm": "feature_projection.layer_norm",
        "w2v_model.layer_norm": "feature_projection.layer_norm",
        "mask_emb": "masked_spec_embed",
    }

    hf_dict = hf_model.state_dict()
    new_dict = {}

    for name, value in fairseq_dict.items():
        if "conv_layers" in name:
            parts = name.split(".")
            layer_idx = int(parts[2])
            type_idx = int(parts[3])
            weight_type = parts[4]

            if type_idx == 0:
                mapped = f"feature_extractor.conv_layers.{layer_idx}.conv.{weight_type}"
            elif type_idx == 2:
                mapped = f"feature_extractor.conv_layers.{layer_idx}.layer_norm.{weight_type}"
            else:
                continue
            new_dict[mapped] = value
        else:
            for k, v in mapping.items():
                if k in name:
                    if "*" in v:
                        layer_idx = name.split(k)[0].split(".")[-2]
                        v = v.replace("*", layer_idx)

                    weight_type = name.split(".")[-1]
                    if weight_type == "weight_g":
                        mapped = f"{v}.parametrizations.weight.original0"
                    elif weight_type == "weight_v":
                        mapped = f"{v}.parametrizations.weight.original1"
                    elif weight_type in HUBERT_WEIGHT_PARAM_NAMES:
                        mapped = f"{v}.{weight_type}"
                    else:
                        mapped = v

                    new_dict[mapped] = value
                    break

    for k, v in new_dict.items():
        if k in hf_dict:
            if hf_dict[k].shape == v.shape:
                if save_dtype is not None and v.is_floating_point():
                    v = v.to(save_dtype)
                hf_dict[k] = v

    if save_dtype is not None:
        hf_dict = {
            k: v.to(save_dtype) if v.is_floating_point() else v
            for k, v in hf_dict.items()
        }

    save_file(hf_dict, safetensors_path)
    model_path.unlink(missing_ok=True)


def download_hubert_model(model_path: Path = HUBERT_BASE_MODEL_PATH) -> Path:
    if model_path.exists():
        return model_path

    model_path.parent.mkdir(parents=True, exist_ok=True)
    return Path(
        hf_hub_download(
            repo_id=LEGACY_RVC_REPO_ID,
            filename=model_path.name,
            local_dir=model_path.parent,
        )
    )


def hubert_large_pruned_model_path(output_layer: int) -> Path:
    if output_layer == 12:
        return HUBERT_LARGE_PRUNED_MODEL_PATH
    return (
        HUBERT_LARGE_PRUNED_MODEL_DIR / f"hubert_large_layer{output_layer}.safetensors"
    )


def hubert_large_feature_config(
    output_layer: int = 12,
) -> HubertConfig:
    return HubertConfig(
        conv_bias=True,
        do_stable_layer_norm=True,
        feat_extract_norm="layer",
        feat_proj_dropout=0.1,
        hidden_size=HUBERT_LARGE_FEATURE_DIM,
        intermediate_size=4096,
        num_attention_heads=16,
        num_hidden_layers=output_layer,
    )


def _keep_hubert_large_feature_weight(name: str, output_layer: int) -> bool:
    if name.startswith("encoder.layer_norm."):
        return False

    prefix = "encoder.layers."
    if not name.startswith(prefix):
        return True

    layer_name = name.removeprefix(prefix)
    layer_id = int(layer_name.split(".", maxsplit=1)[0])
    return layer_id < output_layer


def save_pruned_hubert_large_model(
    safetensors_path: Path | None = None,
    *,
    output_layer: int = 12,
) -> Path:
    safetensors_path = safetensors_path or hubert_large_pruned_model_path(output_layer)
    if safetensors_path.exists():
        return safetensors_path

    logger.info(
        "Downloading HuBERT large from %s to extract layers 0-%s into %s",
        HUBERT_LARGE_MODEL_ID,
        output_layer - 1,
        safetensors_path,
    )
    safetensors_path.parent.mkdir(parents=True, exist_ok=True)
    full_model = HubertForCTC.from_pretrained(HUBERT_LARGE_MODEL_ID).hubert
    pruned_state_dict = {
        name: value.detach().cpu().contiguous()
        for name, value in full_model.state_dict().items()
        if _keep_hubert_large_feature_weight(name, output_layer)
    }
    save_file(pruned_state_dict, safetensors_path)
    return safetensors_path


def load_pruned_hubert_large_model(
    safetensors_path: Path | None = None,
    *,
    output_layer: int = 12,
) -> HubertModel:
    safetensors_path = safetensors_path or hubert_large_pruned_model_path(output_layer)
    hf_model = HubertModel(hubert_large_feature_config(output_layer))
    hf_model.encoder.add_module("layer_norm", nn.Identity())
    missing_keys, unexpected_keys = load_model(hf_model, safetensors_path, strict=False)
    unexpected_key_set = set(unexpected_keys)
    if missing_keys or unexpected_key_set - HUBERT_ALLOWED_UNEXPECTED_KEYS:
        raise RuntimeError(
            "Could not load pruned HuBERT large checkpoint: "
            f"missing={missing_keys}, unexpected={unexpected_keys}"
        )
    return hf_model


@lru_cache(maxsize=1)
def get_hubert() -> HubertModelWrapper:
    from api.pneuma.inference.model_assets import resolve_safetensors_asset

    def download_wrapper(dest_dir: Path) -> Path:
        return Path(
            hf_hub_download(
                repo_id=LEGACY_RVC_REPO_ID,
                filename=HUBERT_BASE_MODEL_PATH.name,
                local_dir=dest_dir,
            )
        )

    safetensors_path = resolve_safetensors_asset(
        safetensors_path=HUBERT_BASE_MODEL_PATH.with_suffix(".safetensors"),
        convert_legacy=convert_fairseq_to_hf,
        asset_name="HuBERT base",
        download_legacy=download_wrapper,
    )

    hf_config = HubertConfig()
    hf_model = HubertModel(hf_config)
    load_model(hf_model, safetensors_path, device=str(get_device()))

    if use_half_precision():
        try:
            hf_model = hf_model.half()
        except Exception as e:
            logger.warning(
                "Could not convert HuBERT to half; keeping float32. Error: %s",
                e,
            )
            hf_model = hf_model.float()
    else:
        hf_model = hf_model.float()

    wrapper = HubertModelWrapper(
        hf_model,
        feature_dim=HUBERT_BASE_FEATURE_DIM,
        default_output_layer=12,
    )
    wrapper.eval()
    return wrapper.to(get_device()).eval()


@lru_cache(maxsize=2)
def get_hubert_large(
    output_layer: int = 12,
) -> HubertModelWrapper:
    safetensors_path = hubert_large_pruned_model_path(output_layer)
    if not safetensors_path.exists():
        save_pruned_hubert_large_model(output_layer=output_layer)
    logger.info("Loading pruned HuBERT large model from %s", safetensors_path)
    hf_model = load_pruned_hubert_large_model(
        safetensors_path=safetensors_path,
        output_layer=output_layer,
    )

    if use_half_precision():
        try:
            hf_model = hf_model.half()
        except Exception as e:
            logger.warning(
                "Could not convert HuBERT large to half; keeping float32. Error: %s",
                e,
            )
            hf_model = hf_model.float()
    else:
        hf_model = hf_model.float()

    wrapper = HubertModelWrapper(
        hf_model,
        feature_dim=HUBERT_LARGE_FEATURE_DIM,
        default_output_layer=output_layer,
    )
    wrapper.eval()
    return wrapper.to(get_device()).eval()
