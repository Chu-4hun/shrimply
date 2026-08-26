from collections.abc import Callable
from importlib import import_module
from importlib.machinery import ModuleSpec
import sys
from types import ModuleType
from typing import ParamSpec, TypeVar

import torch
from torch.nn.attention.flex_attention import BlockMask
from transformers.cache_utils import Cache
from transformers.configuration_utils import ALLOWED_LAYER_TYPES, PreTrainedConfig
from transformers.masking_utils import (
    create_causal_mask,
    create_sliding_window_causal_mask,
)
from transformers.modeling_rope_utils import RotaryEmbeddingConfigMixin
from transformers.models.mimi.modeling_mimi import MimiTransformerModel
import transformers.utils as transformers_utils
import transformers.utils.generic as transformers_generic
from transformers.utils.generic import merge_with_config_defaults

Parameters = ParamSpec("Parameters")
Result = TypeVar("Result")
Documented = TypeVar("Documented")

# Qwen imports its unused 25 Hz tokenizer eagerly. The advertised 12 Hz models do
# not call either dependency, so do not install their native runtime stack.
for dependency in ("onnxruntime", "sox"):
    module = ModuleType(dependency)
    module.__spec__ = ModuleSpec(dependency, loader=None)
    sys.modules[dependency] = module


def qwen_model_inputs(
    func: Callable[Parameters, Result] | None = None,
) -> (
    Callable[Parameters, Result]
    | Callable[[Callable[Parameters, Result]], Callable[Parameters, Result]]
):
    if func is None:
        return merge_with_config_defaults
    return merge_with_config_defaults(func)


def qwen_auto_docstring(
    obj: Documented | None = None,
    *,
    custom_intro: str | None = None,
    custom_args: str | None = None,
    checkpoint: str | None = None,
) -> Documented | Callable[[Documented], Documented]:
    del custom_intro, custom_args, checkpoint
    if obj is not None:
        return obj
    return lambda documented: documented


# Qwen uses the old decorator-factory spelling during import. Restore the
# Transformers symbol immediately; all persistent patches below target Qwen.
transformers_check_model_inputs = transformers_generic.check_model_inputs
transformers_auto_docstring = transformers_utils.auto_docstring
transformers_generic.check_model_inputs = qwen_model_inputs
transformers_utils.auto_docstring = qwen_auto_docstring
qwen_tts = import_module("qwen_tts")
transformers_generic.check_model_inputs = transformers_check_model_inputs
transformers_utils.auto_docstring = transformers_auto_docstring

Qwen3TTSModel = qwen_tts.Qwen3TTSModel
VoiceClonePromptItem = qwen_tts.VoiceClonePromptItem
qwen_configuration = import_module(
    "qwen_tts.core.models.configuration_qwen3_tts"
)
Qwen3TTSConfig = qwen_configuration.Qwen3TTSConfig
Qwen3TTSTalkerCodePredictorConfig = (
    qwen_configuration.Qwen3TTSTalkerCodePredictorConfig
)
Qwen3TTSTalkerConfig = qwen_configuration.Qwen3TTSTalkerConfig
qwen_modeling = import_module("qwen_tts.core.models.modeling_qwen3_tts")
Qwen3TTSRotaryEmbedding = qwen_modeling.Qwen3TTSRotaryEmbedding
Qwen3TTSTalkerForConditionalGeneration = (
    qwen_modeling.Qwen3TTSTalkerForConditionalGeneration
)
Qwen3TTSTalkerOutputWithPast = qwen_modeling.Qwen3TTSTalkerOutputWithPast
Qwen3TTSTalkerRotaryEmbedding = qwen_modeling.Qwen3TTSTalkerRotaryEmbedding
qwen_tokenizer_configuration = import_module(
    "qwen_tts.core.tokenizer_12hz.configuration_qwen3_tts_tokenizer_v2"
)
Qwen3TTSTokenizerV2DecoderConfig = (
    qwen_tokenizer_configuration.Qwen3TTSTokenizerV2DecoderConfig
)
qwen_tokenizer_modeling = import_module(
    "qwen_tts.core.tokenizer_12hz.modeling_qwen3_tts_tokenizer_v2"
)
Qwen3TTSTokenizerV2DecoderRotatoryEmbedding = (
    qwen_tokenizer_modeling.Qwen3TTSTokenizerV2DecoderRotatoryEmbedding
)
Qwen3TTSTokenizer = import_module(
    "qwen_tts.inference.qwen3_tts_tokenizer"
).Qwen3TTSTokenizer


def validate_rope(
    config: RotaryEmbeddingConfigMixin,
    ignore_keys: set[str] | None = None,
) -> None:
    del ignore_keys
    config.standardize_rope_params()
    if not isinstance(config, PreTrainedConfig):
        raise TypeError("Qwen rope configuration is not a Transformers config")
    config.validate_rope()


def validate_layer_types(
    layer_types: list[str],
    num_hidden_layers: int | None = None,
    attention: bool = True,
) -> None:
    del attention
    if not all(layer_type in ALLOWED_LAYER_TYPES for layer_type in layer_types):
        raise ValueError(f"The layer type entries must be in {ALLOWED_LAYER_TYPES}")
    if num_hidden_layers is not None and num_hidden_layers != len(layer_types):
        raise ValueError(
            f"num_hidden_layers ({num_hidden_layers}) must equal the number of "
            f"layer types ({len(layer_types)})"
        )


def default_rope(
    config: Qwen3TTSConfig
    | Qwen3TTSTalkerConfig
    | Qwen3TTSTalkerCodePredictorConfig
    | Qwen3TTSTokenizerV2DecoderConfig,
    device: torch.device | None = None,
    seq_len: int | None = None,
    layer_type: str | None = None,
) -> tuple[torch.Tensor, float]:
    del seq_len, layer_type
    configured_head_dimension = getattr(config, "head_dim", None)
    head_dimension = (
        configured_head_dimension
        if isinstance(configured_head_dimension, int)
        else config.hidden_size // config.num_attention_heads
    )
    frequencies = torch.arange(
        0, head_dimension, 2, dtype=torch.int64, device=device
    ).float()
    return 1.0 / (config.rope_theta ** (frequencies / head_dimension)), 1.0


type MimiForwardValue = torch.Tensor | Cache | bool | None


def qwen_mimi_encoder_mask(
    module: MimiTransformerModel,
    inputs: tuple[torch.Tensor, ...],
    kwargs: dict[str, MimiForwardValue],
) -> tuple[tuple[torch.Tensor, ...], dict[str, MimiForwardValue]]:
    if not inputs:
        raise RuntimeError("Qwen Mimi encoder received no hidden states")
    hidden_states = inputs[0]
    past_key_values = kwargs.get("past_key_values")
    past_length = (
        past_key_values.get_seq_length() if isinstance(past_key_values, Cache) else 0
    )
    positions = torch.arange(
        past_length,
        past_length + hidden_states.shape[1],
        device=hidden_states.device,
    )
    keys = torch.arange(
        past_length + hidden_states.shape[1], device=hidden_states.device
    )
    kwargs["attention_mask"] = (
        (keys <= positions[:, None])
        & (keys > positions[:, None] - module.config.sliding_window)
    )[None, None].expand(hidden_states.shape[0], -1, -1, -1)
    return inputs, kwargs


def prepare_qwen_model(model: Qwen3TTSModel) -> None:
    speech_tokenizer = model.model.speech_tokenizer
    if not isinstance(speech_tokenizer, Qwen3TTSTokenizer):
        raise TypeError("Qwen model has an invalid speech tokenizer")
    roots = (model.model, speech_tokenizer.model)
    if not all(isinstance(root, torch.nn.Module) for root in roots):
        raise TypeError("Qwen model has invalid module roots")
    for root in roots:
        for module in root.modules():
            if isinstance(module, MimiTransformerModel):
                module.register_forward_pre_hook(
                    qwen_mimi_encoder_mask, with_kwargs=True
                )
                continue
            if isinstance(
                module,
                (
                    Qwen3TTSRotaryEmbedding,
                    Qwen3TTSTalkerRotaryEmbedding,
                    Qwen3TTSTokenizerV2DecoderRotatoryEmbedding,
                ),
            ):
                inverse_frequencies = module.inv_freq
                if not isinstance(inverse_frequencies, torch.Tensor):
                    raise TypeError("Qwen rotary embedding has invalid frequencies")
                frequencies, module.attention_scaling = default_rope(
                    module.config, inverse_frequencies.device
                )
                inverse_frequencies.copy_(frequencies)
                module.original_inv_freq = inverse_frequencies


original_repeat_key_value_heads = qwen_modeling.repeat_kv


# Materialize BF16 grouped-query heads: a zero batch stride makes cuBLAS reject
# strided batched matrix multiplication when the model has one key/value head.
def repeat_key_value_heads(hidden_states: torch.Tensor, n_rep: int) -> torch.Tensor:
    if hidden_states.dtype == torch.bfloat16 and n_rep != 1:
        return torch.repeat_interleave(hidden_states, n_rep, dim=1)
    return original_repeat_key_value_heads(hidden_states, n_rep)


def causal_mask(
    config: PreTrainedConfig,
    input_embeds: torch.Tensor,
    attention_mask: torch.Tensor | None,
    cache_position: torch.Tensor | None,
    past_key_values: Cache | None,
    position_ids: torch.Tensor | None = None,
) -> torch.Tensor | BlockMask | None:
    del cache_position
    return create_causal_mask(
        config,
        input_embeds,
        attention_mask,
        past_key_values,
        position_ids,
    )


def tokenizer_causal_mask(
    config: Qwen3TTSTokenizerV2DecoderConfig,
    input_embeds: torch.Tensor,
    attention_mask: torch.Tensor | None,
    cache_position: torch.Tensor | None,
    past_key_values: Cache | None,
    position_ids: torch.Tensor | None = None,
) -> torch.Tensor | BlockMask | None:
    if attention_mask is not None:
        return create_causal_mask(
            config,
            input_embeds,
            attention_mask,
            past_key_values,
            position_ids,
        )
    positions = (
        cache_position
        if cache_position is not None
        else torch.arange(input_embeds.shape[1], device=input_embeds.device)
    )
    key_count = input_embeds.shape[1] + (
        past_key_values.get_seq_length() if past_key_values is not None else 0
    )
    return (torch.arange(key_count, device=input_embeds.device) <= positions[:, None])[
        None, None
    ].expand(input_embeds.shape[0], -1, -1, -1)


def tokenizer_sliding_window_causal_mask(
    config: Qwen3TTSTokenizerV2DecoderConfig,
    input_embeds: torch.Tensor,
    attention_mask: torch.Tensor | None,
    cache_position: torch.Tensor | None,
    past_key_values: Cache | None,
    position_ids: torch.Tensor | None = None,
) -> torch.Tensor | BlockMask | None:
    if attention_mask is not None:
        return create_sliding_window_causal_mask(
            config,
            input_embeds,
            attention_mask,
            past_key_values,
            position_ids,
        )
    positions = (
        cache_position
        if cache_position is not None
        else torch.arange(input_embeds.shape[1], device=input_embeds.device)
    )
    key_count = input_embeds.shape[1] + (
        past_key_values.get_seq_length() if past_key_values is not None else 0
    )
    keys = torch.arange(key_count, device=input_embeds.device)
    return (
        (keys <= positions[:, None])
        & (keys > positions[:, None] - config.sliding_window)
    )[None, None].expand(input_embeds.shape[0], -1, -1, -1)


def sliding_window_causal_mask(
    config: PreTrainedConfig,
    input_embeds: torch.Tensor,
    attention_mask: torch.Tensor | None,
    cache_position: torch.Tensor | None,
    past_key_values: Cache | None,
    position_ids: torch.Tensor | None = None,
) -> torch.Tensor | BlockMask | None:
    del cache_position
    return create_sliding_window_causal_mask(
        config,
        input_embeds,
        attention_mask,
        past_key_values,
        position_ids,
    )


qwen_talker_forward = Qwen3TTSTalkerForConditionalGeneration.forward


def qwen_talker_forward_with_cache_position(
    self: Qwen3TTSTalkerForConditionalGeneration,
    input_ids: torch.Tensor | None = None,
    attention_mask: torch.Tensor | None = None,
    position_ids: torch.Tensor | None = None,
    past_key_values: Cache | None = None,
    inputs_embeds: torch.Tensor | None = None,
    labels: torch.Tensor | None = None,
    use_cache: bool | None = None,
    output_attentions: bool | None = None,
    output_hidden_states: bool | None = None,
    cache_position: torch.Tensor | None = None,
    past_hidden: torch.Tensor | None = None,
    trailing_text_hidden: torch.Tensor | None = None,
    tts_pad_embed: torch.Tensor | None = None,
    generation_step: int | None = None,
    subtalker_dosample: bool | None = None,
    subtalker_top_p: float | None = None,
    subtalker_top_k: int | None = None,
    subtalker_temperature: float | None = None,
    return_dict: bool | None = None,
) -> Qwen3TTSTalkerOutputWithPast:
    sequence = inputs_embeds if inputs_embeds is not None else input_ids
    if cache_position is None and sequence is not None:
        past_length = (
            past_key_values.get_seq_length() if past_key_values is not None else 0
        )
        cache_position = torch.arange(
            past_length,
            past_length + sequence.shape[1],
            device=sequence.device,
        )
    output = qwen_talker_forward(
        self,
        input_ids=input_ids,
        attention_mask=attention_mask,
        position_ids=position_ids,
        past_key_values=past_key_values,
        inputs_embeds=inputs_embeds,
        labels=labels,
        use_cache=use_cache,
        output_attentions=output_attentions,
        output_hidden_states=output_hidden_states,
        cache_position=cache_position,
        past_hidden=past_hidden,
        trailing_text_hidden=trailing_text_hidden,
        tts_pad_embed=tts_pad_embed,
        generation_step=generation_step,
        subtalker_dosample=subtalker_dosample,
        subtalker_top_p=subtalker_top_p,
        subtalker_top_k=subtalker_top_k,
        subtalker_temperature=subtalker_temperature,
        return_dict=return_dict,
    )
    if not isinstance(output, Qwen3TTSTalkerOutputWithPast):
        raise TypeError("Qwen talker returned an invalid model output")
    return output


# Keep every compatibility change on Qwen's imported symbols. Transformers 5 is
# left untouched for the rest of the server.
setattr(qwen_configuration, "rope_config_validation", validate_rope)
setattr(qwen_configuration, "layer_type_validation", validate_layer_types)
setattr(qwen_modeling, "repeat_kv", repeat_key_value_heads)
setattr(qwen_tokenizer_modeling, "repeat_kv", repeat_key_value_heads)
Qwen3TTSTalkerConfig.pad_token_id = None
Qwen3TTSTalkerCodePredictorConfig.pad_token_id = None
setattr(
    Qwen3TTSTalkerForConditionalGeneration,
    "forward",
    qwen_talker_forward_with_cache_position,
)
setattr(
    qwen_modeling,
    "ROPE_INIT_FUNCTIONS",
    {**getattr(qwen_modeling, "ROPE_INIT_FUNCTIONS"), "default": default_rope},
)
setattr(
    qwen_tokenizer_modeling,
    "ROPE_INIT_FUNCTIONS",
    {
        **getattr(qwen_tokenizer_modeling, "ROPE_INIT_FUNCTIONS"),
        "default": default_rope,
    },
)
setattr(qwen_modeling, "create_causal_mask", causal_mask)
setattr(qwen_modeling, "create_sliding_window_causal_mask", sliding_window_causal_mask)
setattr(qwen_tokenizer_modeling, "create_causal_mask", tokenizer_causal_mask)
setattr(
    qwen_tokenizer_modeling,
    "create_sliding_window_causal_mask",
    tokenizer_sliding_window_causal_mask,
)

__all__ = ["Qwen3TTSModel", "VoiceClonePromptItem", "prepare_qwen_model"]
