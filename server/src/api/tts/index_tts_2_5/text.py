import base64
import re
from collections.abc import Iterable, Mapping, MutableMapping
from functools import cache
from importlib import import_module
from pathlib import Path
from typing import Protocol, cast

import tiktoken

from api.tts.index_tts_2_0.text import GlossaryTerm, normalize_text


LANGUAGES = (
    "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl",
    "ca", "nl", "ar", "sv", "it", "id", "hi", "fi", "vi", "he", "uk",
    "el", "ms", "cs", "ro", "da", "hu", "ta", "no", "th", "ur", "hr",
    "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn",
    "sr", "az", "sl", "kn", "et", "mk", "br", "eu", "is", "hy", "ne",
    "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si", "km", "sn",
    "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi",
    "lo", "uz", "fo", "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my",
    "bo", "tl", "mg", "as", "tt", "haw", "ln", "ha", "ba", "jw", "su",
    "yue", "minnan", "wuyu", "dialect", "zh/en", "en/zh", "common",
)
_SUPPORTED_LANGUAGES = frozenset(("zh", "en", "ja", "es", "ar"))
_AUDIO_EVENTS = (
    "ASR", "AED", "SER", "Speech", "/Speech", "BGM", "/BGM", "Laughter",
    "/Laughter", "Applause", "/Applause",
)
_EMOTIONS = ("HAPPY", "SAD", "ANGRY", "NEUTRAL")
_TTS_TOKENS = (
    "TTS/B", "TTS/O", "TTS/Q", "TTS/A", "TTS/CO", "TTS/CL", "TTS/H",
    *(f"TTS/SP{index:02d}" for index in range(1, 14)),
)
_PRONUNCIATION_PATTERN = re.compile(r"<([^|>\n]+)\|([^>\n]+)>")
_PROTECTED_PATTERN = re.compile(r"<\|SPECIAL_TOKEN_\d+\|>.*?<\|SPECIAL_TOKEN_\d+\|>")
_SPLIT_PATTERN = re.compile(r"(?<=[，。！？、；：,\.!?;:\n])")
_CHARACTER_REPLACEMENTS = str.maketrans(
    {
        "：": ",", "；": ",", ";": ",", "，": ",", "。": ".",
        "！": "!", "？": "?", "\n": " ", "·": "-", "、": ",",
        "“": "'", "”": "'", '"': "'", "‘": "'", "’": "'",
        "（": "'", "）": "'", "(": "'", ")": "'", "《": "'",
        "》": "'", "【": "'", "】": "'", "[": "'", "]": "'",
        "—": "-", "～": "-", "~": "-", "「": "'", "」": "'", ":": ",",
    }
)


class _JapaneseWord(Protocol):
    surface: str


class _JapaneseTagger(Protocol):
    def __call__(self, text: str) -> Iterable[_JapaneseWord]: ...


class _FugashiModule(Protocol):
    def Tagger(self) -> _JapaneseTagger: ...


class _SpanishNormalizer(Protocol):
    def normalize(self, text: str, *, verbose: bool) -> str: ...


class _NemoNormalizeModule(Protocol):
    def Normalizer(
        self,
        *,
        input_case: str,
        lang: str,
        cache_dir: str,
        overwrite_cache: bool,
    ) -> _SpanishNormalizer: ...


class MultilingualTextTokenizer:
    def __init__(self, model_path: Path, normalization_cache: Path) -> None:
        ranks = {
            base64.b64decode(token): int(rank)
            for token, rank in (
                line.split() for line in model_path.read_bytes().splitlines() if line
            )
        }
        special_values = (
            "<|endoftext|>",
            "<|startoftranscript|>",
            *(f"<|{language}|>" for language in LANGUAGES[:99]),
            *(f"<|{event}|>" for event in _AUDIO_EVENTS),
            *(f"<|{emotion}|>" for emotion in _EMOTIONS),
            "<|translate|>", "<|transcribe|>", "<|startoflm|>",
            "<|startofprev|>", "<|nospeech|>", "<|notimestamps|>",
            *(f"<|SPECIAL_TOKEN_{index}|>" for index in range(1, 31)),
            *(f"<|{token}|>" for token in _TTS_TOKENS),
            *(f"<|{index * 0.02:.2f}|>" for index in range(1_501)),
        )
        special_tokens = {
            token: len(ranks) + index for index, token in enumerate(special_values)
        }
        self._encoding = tiktoken.Encoding(
            name=model_path.stem,
            explicit_n_vocab=len(ranks) + len(special_tokens),
            pat_str=r"'s|'t|'re|'ve|'m|'ll|'d| ?\p{L}+| ?\p{N}+| ?[^\s\p{L}\p{N}]+|\s+(?!\S)|\s+",
            mergeable_ranks=ranks,
            special_tokens=special_tokens,
        )
        self._normalization_cache = normalization_cache
        fugashi = cast(_FugashiModule, import_module("fugashi"))
        self._japanese_tagger = fugashi.Tagger()

    def segments(
        self,
        text: str,
        glossary: list[GlossaryTerm],
        language: str,
        maximum_tokens: int,
    ) -> list[list[int]]:
        if language not in _SUPPORTED_LANGUAGES:
            raise ValueError(f"IndexTTS 2.5 does not support language {language!r}")
        normalized = self._normalize(text, glossary, language)
        prefix = f"<|{language}|> "
        prefix_tokens = self._encode(prefix)
        budget = min(maximum_tokens, 598) - len(prefix_tokens)
        if budget < 1:
            raise ValueError("Maximum text tokens are too small for the language prefix")
        return [
            [*prefix_tokens, *self._encode(segment), 1]
            for segment in self._split(normalized, budget)
        ]

    def language_index(self, language: str) -> int:
        try:
            return LANGUAGES.index(language)
        except ValueError:
            return LANGUAGES.index("common")

    def _normalize(
        self, text: str, glossary: list[GlossaryTerm], language: str
    ) -> str:
        protected, annotations = _protect_annotations(text)
        if language in ("zh", "en"):
            normalized = normalize_text(
                protected,
                glossary,
                self._normalization_cache,
            )
        else:
            normalized = protected
            for entry in sorted(glossary, key=lambda value: len(value.term), reverse=True):
                replacement = entry.english
                if replacement is not None:
                    normalized = re.sub(
                        re.escape(entry.term), replacement, normalized, flags=re.IGNORECASE
                    )
            normalized = normalized.translate(_CHARACTER_REPLACEMENTS)
            if language == "es":
                normalized = _spanish_normalizer(
                    self._normalization_cache
                ).normalize(normalized, verbose=False)
        for placeholder, annotation in annotations.items():
            normalized = normalized.replace(placeholder, annotation)
        if language in ("ja", "zh", "en"):
            normalized = normalized.lower()
        elif language == "es":
            normalized = normalized.upper()
        normalized = _PRONUNCIATION_PATTERN.sub(_replace_pronunciation, normalized)
        if language == "ja":
            return _segment_japanese(normalized, self._japanese_tagger)
        return normalized

    def _split(self, text: str, budget: int) -> list[str]:
        if len(self._encode(text)) <= budget:
            return [text]
        chunks: list[str] = []
        position = 0
        for match in _PROTECTED_PATTERN.finditer(text):
            chunks.extend(_split_piece(text[position : match.start()], self, budget))
            chunks.append(match.group())
            position = match.end()
        chunks.extend(_split_piece(text[position:], self, budget))
        segments: list[str] = []
        current = ""
        for chunk in chunks:
            if current and len(self._encode(current + chunk)) > budget:
                segments.append(current)
                current = chunk
            else:
                current += chunk
        if current:
            segments.append(current)
        return segments

    def _encode(self, text: str) -> list[int]:
        return self._encoding.encode(text, allowed_special="all")


def _protect_annotations(text: str) -> tuple[str, Mapping[str, str]]:
    annotations: MutableMapping[str, str] = {}

    def replace(match: re.Match[str]) -> str:
        index = len(annotations)
        suffix = ""
        while True:
            suffix = chr(ord("A") + index % 26) + suffix
            index = index // 26 - 1
            if index < 0:
                break
        placeholder = f"PRONUNCIATIONPLACEHOLDER{suffix}"
        annotations[placeholder] = match.group()
        return placeholder

    return _PRONUNCIATION_PATTERN.sub(replace, text), annotations


def _replace_pronunciation(match: re.Match[str]) -> str:
    word, pronunciation = match.groups()
    if re.fullmatch(r"[\u3040-\u30ff]+", pronunciation):
        return f" {pronunciation} "
    token = "SPECIAL_TOKEN_2" if re.search(r"[\u4e00-\u9fff]", word) else "SPECIAL_TOKEN_1"
    value = pronunciation.upper()
    return f"<|{token}|>{value}<|{token}|>"


def _segment_japanese(text: str, tagger: _JapaneseTagger) -> str:
    return "".join(
        whitespace
        if whitespace
        else " ".join(word.surface for word in tagger(segment))
        for segment, whitespace in re.findall(r"([^ ]+)|( +)", text)
    )


@cache
def _spanish_normalizer(cache_directory: Path) -> _SpanishNormalizer:
    module = cast(
        _NemoNormalizeModule,
        import_module("nemo_text_processing.text_normalization.normalize"),
    )
    cache_directory.mkdir(parents=True, exist_ok=True)
    return module.Normalizer(
        input_case="cased",
        lang="es",
        cache_dir=str(cache_directory),
        overwrite_cache=False,
    )


def _split_piece(
    text: str, tokenizer: MultilingualTextTokenizer, budget: int
) -> list[str]:
    chunks: list[str] = []
    for part in _SPLIT_PATTERN.split(text):
        if not part:
            continue
        if len(tokenizer._encode(part)) <= budget:
            chunks.append(part)
            continue
        current = ""
        for character in part:
            if current and len(tokenizer._encode(current + character)) > budget:
                chunks.append(current)
                current = character
            else:
                current += character
        if current:
            chunks.append(current)
    return chunks
