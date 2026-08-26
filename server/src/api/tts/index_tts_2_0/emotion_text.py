import json
import re
from functools import cache
from pathlib import Path

import torch
from pydantic import BaseModel, ConfigDict, Field
from torch import Tensor
from transformers import AutoModelForCausalLM, AutoTokenizer


class EmotionScores(BaseModel):
    model_config = ConfigDict(extra="ignore", strict=True)

    happy: float = Field(default=0.0, alias="高兴", ge=0.0, le=1.2)
    angry: float = Field(default=0.0, alias="愤怒", ge=0.0, le=1.2)
    sad: float = Field(default=0.0, alias="悲伤", ge=0.0, le=1.2)
    afraid: float = Field(default=0.0, alias="恐惧", ge=0.0, le=1.2)
    disgusted: float = Field(default=0.0, alias="反感", ge=0.0, le=1.2)
    melancholic: float = Field(default=0.0, alias="低落", ge=0.0, le=1.2)
    surprised: float = Field(default=0.0, alias="惊讶", ge=0.0, le=1.2)
    calm: float = Field(default=0.0, alias="自然", ge=0.0, le=1.2)

    def in_model_order(self) -> list[float]:
        return [
            self.happy,
            self.angry,
            self.sad,
            self.afraid,
            self.disgusted,
            self.melancholic,
            self.surprised,
            self.calm,
        ]


_FALLBACK_SCORE_PATTERN = re.compile(r'([^\s":.,]+?)"?\s*:\s*([\d.]+)')
_MELANCHOLIC_WORDS = frozenset(
    (
        "低落",
        "melancholy",
        "melancholic",
        "depression",
        "depressed",
        "gloomy",
    )
)
_END_THINK_TOKEN = 151_668
_MAX_GENERATED_TOKENS = 32_768


class EmotionTextAnalyzer:
    def __init__(
        self, model_directory: Path, device: torch.device, dtype: torch.dtype
    ) -> None:
        tokenizer = AutoTokenizer.from_pretrained(
            model_directory, local_files_only=True
        )
        if tokenizer is None:
            raise RuntimeError("Could not load the emotion tokenizer")
        self._tokenizer = tokenizer
        model = AutoModelForCausalLM.from_pretrained(
            model_directory,
            local_files_only=True,
            dtype=dtype,
        ).to(device)
        model.eval()
        self._model = model

    @torch.inference_mode()
    def analyze(self, text: str) -> list[float]:
        messages = [
            {"role": "system", "content": "文本情感分类"},
            {"role": "user", "content": text},
        ]
        prompt = self._tokenizer.apply_chat_template(
            messages,
            tokenize=False,
            add_generation_prompt=True,
            enable_thinking=False,
        )
        if not isinstance(prompt, str):
            raise RuntimeError("Emotion tokenizer did not produce a text prompt")
        encoded = self._tokenizer([prompt], return_tensors="pt")
        input_ids = encoded["input_ids"]
        attention_mask = encoded["attention_mask"]
        if not isinstance(input_ids, Tensor) or not isinstance(attention_mask, Tensor):
            raise RuntimeError("Emotion tokenizer did not produce tensor inputs")
        input_ids = input_ids.to(self._model.device)
        attention_mask = attention_mask.to(self._model.device)
        eos_token = self._tokenizer.eos_token_id
        if eos_token is None:
            raise RuntimeError("Emotion tokenizer has no end token")
        generate = getattr(self._model, "generate", None)
        if not callable(generate):
            raise TypeError("Emotion model does not support generation")
        generated = generate(
            input_ids,
            attention_mask=attention_mask,
            max_new_tokens=_MAX_GENERATED_TOKENS,
            pad_token_id=eos_token,
        )
        if not isinstance(generated, Tensor):
            raise TypeError("Emotion model returned invalid token IDs")
        output = generated[0, input_ids.shape[1] :].tolist()
        try:
            content_start = len(output) - output[::-1].index(_END_THINK_TOKEN)
        except ValueError:
            content_start = 0
        decoded = self._tokenizer.decode(
            output[content_start:], skip_special_tokens=True
        )
        if not isinstance(decoded, str):
            raise RuntimeError("Emotion tokenizer returned multiple responses")
        scores = _parse_scores(decoded)
        if any(word in text.lower() for word in _MELANCHOLIC_WORDS):
            scores.sad, scores.melancholic = scores.melancholic, scores.sad
        values = scores.in_model_order()
        if not any(value > 0 for value in values):
            values[-1] = 1.0
        return values


def _parse_scores(content: str) -> EmotionScores:
    try:
        decoded = json.loads(content)
    except json.JSONDecodeError:
        decoded = {
            match.group(1): float(match.group(2))
            for match in _FALLBACK_SCORE_PATTERN.finditer(content)
        }
    if not isinstance(decoded, dict):
        raise ValueError("Emotion model response must be a JSON mapping")
    return EmotionScores.model_validate(decoded)


@cache
def load_emotion_text_analyzer(
    model_directory: Path, device: torch.device, dtype: torch.dtype
) -> EmotionTextAnalyzer:
    return EmotionTextAnalyzer(model_directory, device, dtype)
