import re
from dataclasses import dataclass
from functools import cache, lru_cache
from pathlib import Path
from sentencepiece import SentencePieceProcessor
from tn.chinese.normalizer import Normalizer as ChineseNormalizer
from tn.english.normalizer import Normalizer as EnglishNormalizer


@dataclass(frozen=True, slots=True)
class GlossaryTerm:
    term: str
    chinese: str | None
    english: str | None


_PINYIN_PATTERN = re.compile(
    r"(?<![a-z])((?:[bpmfdtnlgkhjqxzcsryw]|[zcs]h)?"
    r"(?:[aeiouüv]|[ae]i|u[aio]|ao|ou|i[aue]|[uüv]e|[uvü]ang?|uai|"
    r"[aeiuv]n|[aeio]ng|ia[no]|i[ao]ng)|ng|er)([1-5])",
    re.IGNORECASE,
)
_NAME_PATTERN = re.compile(r"[\u4e00-\u9fff]+(?:[-·—][\u4e00-\u9fff]+){1,2}")
_TECH_TERM_PATTERN = re.compile(r"[A-Za-z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)+")
_CONTRACTION_PATTERN = re.compile(
    r"(what|where|who|which|how|t?here|it|s?he|that|this)'s", re.IGNORECASE
)
_CHINESE_CHARACTER_PATTERN = re.compile(r"[\u4e00-\u9fff]")
_ALPHABETIC_PATTERN = re.compile(r"[A-Za-z]")
_EMAIL_PATTERN = re.compile(r"^[A-Za-z0-9]+@[A-Za-z0-9]+\.[A-Za-z]+$")
_CJK_RANGE_PATTERN = re.compile(
    r"([\u1100-\u11ff\u2e80-\ua4cf\ua840-\uD7AF\uF900-\uFAFF"
    r"\uFE30-\uFE4F\uFF65-\uFFDC\U00020000-\U0002FFFF])"
)
_CHARACTER_REPLACEMENTS = str.maketrans(
    {
        "：": ",",
        "；": ",",
        ";": ",",
        "，": ",",
        "。": ".",
        "！": "!",
        "？": "?",
        "\n": " ",
        "·": "-",
        "、": ",",
        "“": "'",
        "”": "'",
        '"': "'",
        "‘": "'",
        "’": "'",
        "（": "'",
        "）": "'",
        "(": "'",
        ")": "'",
        "《": "'",
        "》": "'",
        "【": "'",
        "】": "'",
        "[": "'",
        "]": "'",
        "—": "-",
        "～": "-",
        "~": "-",
        "「": "'",
        "」": "'",
        ":": ",",
    }
)


class TextTokenizer:
    _sentence_endings = frozenset((".", "!", "?", "▁.", "▁?", "▁..."))

    def __init__(
        self,
        model_path: Path,
        normalization_cache: Path,
    ) -> None:
        if not model_path.is_file():
            raise FileNotFoundError(f"IndexTTS tokenizer model is missing: {model_path}")
        self._processor = SentencePieceProcessor(model_file=str(model_path))
        self._normalization_cache = normalization_cache

    @property
    def unknown_token_id(self) -> int:
        return self._processor.unk_id()

    def tokenize(self, text: str, glossary: list[GlossaryTerm]) -> list[str]:
        if not text:
            return []
        normalized = (
            text
            if len(text.strip()) == 1
            else normalize_text(text, glossary, self._normalization_cache)
        )
        return self._processor.EncodeAsPieces(_tokenize_cjk(normalized))

    def token_ids(self, tokens: list[str]) -> list[int]:
        return [self._processor.PieceToId(token) for token in tokens]

    def split_segments(
        self, tokens: list[str], maximum_tokens: int
    ) -> list[list[str]]:
        if maximum_tokens < 1:
            raise ValueError("Maximum tokens per segment must be positive")
        segments: list[list[str]] = []
        current: list[str] = []
        for token in tokens:
            current.append(token)
            if (
                token in self._sentence_endings and len(current) > 2
            ) or len(current) == maximum_tokens:
                segments.append(current)
                current = []
        if current:
            segments.append(current)
        merged: list[list[str]] = []
        for segment in segments:
            if merged and len(merged[-1]) + len(segment) <= maximum_tokens:
                merged[-1].extend(segment)
            else:
                merged.append(segment)
        return merged


def _uses_chinese_normalization(text: str) -> bool:
    has_chinese = _CHINESE_CHARACTER_PATTERN.search(text) is not None
    has_alphabetic = _ALPHABETIC_PATTERN.search(text) is not None
    return (
        has_chinese
        or not has_alphabetic
        or _EMAIL_PATTERN.fullmatch(text) is not None
        or _PINYIN_PATTERN.search(text) is not None
    )


def normalize_text(
    text: str,
    glossary: list[GlossaryTerm],
    cache_directory: Path,
) -> str:
    chinese_normalizer, english_normalizer = _normalization_engines(cache_directory)
    chinese = _uses_chinese_normalization(text)
    text = _CONTRACTION_PATTERN.sub(r"\1 is", text)
    text = _apply_glossary(text, glossary, chinese)
    protected_tech: list[str] = sorted(
        {match.group() for match in _TECH_TERM_PATTERN.finditer(text)},
        key=len,
        reverse=True,
    )
    for term in protected_tech:
        text = text.replace(term, term.replace("-", "<H>"))
    if chinese:
        text, names = _protect_matches(text.rstrip(), _NAME_PATTERN, "NAME")
        text, pinyin = _protect_matches(text, _PINYIN_PATTERN, "PINYIN")
        normalized = chinese_normalizer.normalize(text)
        if not isinstance(normalized, str):
            raise RuntimeError("Chinese normalizer returned multiple candidates")
        text = _restore_pinyin(_restore_matches(normalized, names, "NAME"), pinyin)
    else:
        normalized = english_normalizer.normalize(text)
        if not isinstance(normalized, str):
            raise RuntimeError("English normalizer returned multiple candidates")
        text = normalized
    return re.sub(r"\s*<H>\s*", "-", text).translate(
        _CHARACTER_REPLACEMENTS
    ).replace("...", "…").replace(",,,", "…")


def _apply_glossary(
    text: str,
    glossary: list[GlossaryTerm],
    chinese: bool,
) -> str:
    for entry in sorted(glossary, key=lambda value: len(value.term), reverse=True):
        replacement = entry.chinese if chinese else entry.english
        if replacement is not None:
            text = _glossary_pattern(entry.term).sub(replacement, text)
    return text


def _protect_matches(
    text: str,
    pattern: re.Pattern[str],
    prefix: str,
) -> tuple[str, list[str]]:
    values = list(dict.fromkeys(match.group() for match in pattern.finditer(text)))
    for index, value in enumerate(values):
        text = text.replace(value, f"<{prefix}{index}>")
    return text, values


def _restore_matches(text: str, values: list[str], prefix: str) -> str:
    for index, value in enumerate(values):
        text = text.replace(f"<{prefix}{index}>", value)
    return text


def _restore_pinyin(text: str, values: list[str]) -> str:
    for index, value in enumerate(values):
        if value[0] in "jqxJQX":
            value = re.sub(
                r"([jqx])[uü](n|e|an)*(\d)",
                r"\g<1>v\g<2>\g<3>",
                value,
                flags=re.IGNORECASE,
            ).upper()
        text = text.replace(f"<PINYIN{index}>", value)
    return text


def _tokenize_cjk(text: str) -> str:
    pieces = _CJK_RANGE_PATTERN.split(text.strip())
    return " ".join(piece.strip().upper() for piece in pieces if piece.strip())


@cache
def _normalization_engines(
    cache_directory: Path,
) -> tuple[ChineseNormalizer, EnglishNormalizer]:
    cache_directory.mkdir(parents=True, exist_ok=True)
    return (
        ChineseNormalizer(
            cache_dir=str(cache_directory),
            remove_interjections=False,
            remove_erhua=False,
            overwrite_cache=False,
        ),
        EnglishNormalizer(overwrite_cache=False),
    )


@lru_cache(maxsize=256)
def _glossary_pattern(term: str) -> re.Pattern[str]:
    return re.compile(re.escape(term), re.IGNORECASE)
