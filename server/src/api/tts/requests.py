from fractions import Fraction
from typing import Annotated, Literal, Self, TypeVar

from pydantic import Field, TypeAdapter, model_validator

from api.tts.catalog import MODELS
from api.tts.protocol import (
    Audio,
    AudioInput,
    AudioValue,
    InputDefinition,
    InputValue,
    ModelInfo,
    NumberInput,
    NumberValue,
    Precision,
    ProtocolModel,
    SelectInput,
    SelectValue,
    SynthesisRequest,
    TableInput,
    TableValue,
    TextInput,
    TextValue,
    ToggleInput,
    ToggleValue,
)

type QwenLanguage = Literal[
    "Auto",
    "Chinese",
    "English",
    "Japanese",
    "Korean",
    "German",
    "French",
    "Russian",
    "Portuguese",
    "Spanish",
    "Italian",
]
type QwenSpeaker = Literal[
    "Vivian",
    "Serena",
    "Uncle_Fu",
    "Dylan",
    "Eric",
    "Ryan",
    "Aiden",
    "Ono_Anna",
    "Sohee",
]
type IndexLanguage = Literal["zh", "en", "ja", "es", "ar"]
type IndexModelId = Literal["IndexTeam/IndexTTS-2", "IndexTeam/IndexTTS-2.5"]
QWEN_LANGUAGE_VALIDATOR = TypeAdapter(QwenLanguage)
QWEN_SPEAKER_VALIDATOR = TypeAdapter(QwenSpeaker)
INDEX_LANGUAGE_VALIDATOR = TypeAdapter(IndexLanguage)
INDEX_MODEL_VALIDATOR = TypeAdapter(IndexModelId)
PRECISION_VALIDATOR = TypeAdapter(Precision)
type CustomVoiceModelId = Literal[
    "Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice",
    "Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice",
]
type VoiceCloneModelId = Literal[
    "Qwen/Qwen3-TTS-12Hz-0.6B-Base",
    "Qwen/Qwen3-TTS-12Hz-1.7B-Base",
]


class QwenCustomVoiceRequest(ProtocolModel):
    mode: Literal["qwen_custom_voice"] = "qwen_custom_voice"
    model: CustomVoiceModelId
    text: str = Field(min_length=1, max_length=4096)
    language: QwenLanguage
    speaker: QwenSpeaker
    instruction: str | None = Field(default=None, max_length=1024)

    @model_validator(mode="after")
    def validate_instruction_support(self) -> Self:
        if self.model.endswith("0.6B-CustomVoice") and self.instruction:
            raise ValueError("The 0.6B Custom Voice model has no instruction input")
        return self


class QwenVoiceCloneRequest(ProtocolModel):
    mode: Literal["qwen_voice_clone"] = "qwen_voice_clone"
    model: VoiceCloneModelId
    text: str = Field(min_length=1, max_length=4096)
    language: QwenLanguage
    reference_audio: Audio
    reference_text: str | None = Field(default=None, max_length=4096)
    audio_only: bool

    @model_validator(mode="after")
    def validate_reference_text(self) -> Self:
        if not self.audio_only and not self.reference_text:
            raise ValueError(
                "A reference transcript is required outside audio-only mode"
            )
        return self


class QwenVoiceDesignRequest(ProtocolModel):
    mode: Literal["qwen_voice_design"] = "qwen_voice_design"
    model: Literal["Qwen/Qwen3-TTS-12Hz-1.7B-VoiceDesign"]
    text: str = Field(min_length=1, max_length=4096)
    language: QwenLanguage
    instruction: str = Field(min_length=1, max_length=1024)


class SpeakerEmotion(ProtocolModel):
    mode: Literal["speaker"] = "speaker"


class AudioEmotion(ProtocolModel):
    mode: Literal["audio"] = "audio"
    audio: Audio
    strength: float = Field(ge=0.0, le=1.0, allow_inf_nan=False)


class EmotionFactors(ProtocolModel):
    happy: float = Field(ge=0.0, le=0.8, allow_inf_nan=False)
    angry: float = Field(ge=0.0, le=0.8, allow_inf_nan=False)
    sad: float = Field(ge=0.0, le=0.8, allow_inf_nan=False)
    afraid: float = Field(ge=0.0, le=0.8, allow_inf_nan=False)
    disgusted: float = Field(ge=0.0, le=0.8, allow_inf_nan=False)
    melancholic: float = Field(ge=0.0, le=0.8, allow_inf_nan=False)
    surprised: float = Field(ge=0.0, le=0.8, allow_inf_nan=False)
    calm: float = Field(ge=0.0, le=0.8, allow_inf_nan=False)

    def values_in_model_order(self) -> list[float]:
        return list(self.model_dump().values())


class FactorEmotion(ProtocolModel):
    mode: Literal["factors"] = "factors"
    factors: EmotionFactors
    strength: float = Field(ge=0.0, le=1.0, allow_inf_nan=False)
    randomize: bool


class TextEmotion(ProtocolModel):
    mode: Literal["text"] = "text"
    text: str | None = Field(default=None, max_length=4096)
    strength: float = Field(ge=0.0, le=1.0, allow_inf_nan=False)
    randomize: bool


type IndexEmotion = Annotated[
    SpeakerEmotion | AudioEmotion | FactorEmotion | TextEmotion,
    Field(discriminator="mode"),
]


class SpeedTiming(ProtocolModel):
    mode: Literal["speed_factor"] = "speed_factor"
    factor: Fraction

    @model_validator(mode="after")
    def validate_factor(self) -> Self:
        if self.factor <= 0:
            raise ValueError("Speed factor must be positive")
        return self


class DurationTiming(ProtocolModel):
    mode: Literal["duration"] = "duration"
    seconds: Fraction

    @model_validator(mode="after")
    def validate_seconds(self) -> Self:
        if self.seconds <= 0:
            raise ValueError("Duration must be positive")
        return self


type IndexTiming = Annotated[SpeedTiming | DurationTiming, Field(discriminator="mode")]


class IndexSampling(ProtocolModel):
    do_sample: bool
    typical_sampling: bool
    typical_mass: float = Field(gt=0.0, lt=1.0, allow_inf_nan=False)
    top_p: float = Field(gt=0.0, le=1.0, allow_inf_nan=False)
    top_k: int = Field(ge=0)
    temperature: float = Field(gt=0.0, allow_inf_nan=False)
    length_penalty: float = Field(allow_inf_nan=False)
    num_beams: int = Field(ge=1)
    repetition_penalty: float = Field(gt=0.0, allow_inf_nan=False)
    max_mel_tokens: int = Field(ge=1, le=1815)


class GlossaryEntry(ProtocolModel):
    term: str = Field(min_length=1, max_length=256)
    chinese: str | None = Field(default=None, min_length=1, max_length=1024)
    english: str | None = Field(default=None, min_length=1, max_length=1024)

    @model_validator(mode="after")
    def validate_pronunciation(self) -> Self:
        if self.chinese is None and self.english is None:
            raise ValueError("A glossary term needs a Chinese or English pronunciation")
        return self


class IndexTts2Request(ProtocolModel):
    mode: Literal["index_tts_2"] = "index_tts_2"
    model: IndexModelId
    text: str = Field(min_length=1, max_length=16_384)
    language: IndexLanguage | None = None
    voice: Audio
    emotion: IndexEmotion
    timing: IndexTiming
    sampling: IndexSampling
    max_text_tokens_per_segment: int = Field(ge=1, le=600)
    intersegment_silence: Fraction
    glossary: list[GlossaryEntry]

    @model_validator(mode="after")
    def validate_silence_precision(self) -> Self:
        if self.model == "IndexTeam/IndexTTS-2.5" and self.language is None:
            raise ValueError("IndexTTS 2.5 requires a language")
        if self.model == "IndexTeam/IndexTTS-2" and self.language is not None:
            raise ValueError("IndexTTS 2 does not accept a language")
        if not 0 <= self.intersegment_silence <= 10:
            raise ValueError("Intersegment silence must be between 0 and 10 seconds")
        if self.intersegment_silence * 1000 % 1:
            raise ValueError("Intersegment silence must resolve to a whole millisecond")
        return self


type ModelRequest = Annotated[
    QwenCustomVoiceRequest
    | QwenVoiceCloneRequest
    | QwenVoiceDesignRequest
    | IndexTts2Request,
    Field(discriminator="mode"),
]

ValueModel = TypeVar(
    "ValueModel",
    TextValue,
    SelectValue,
    AudioValue,
    ToggleValue,
    NumberValue,
    TableValue,
)


def model_info(request: SynthesisRequest) -> ModelInfo:
    for model in MODELS:
        if model.id == request.model:
            return model
    raise ValueError(f"Unknown text-to-speech model {request.model}")


def selected_value(value: InputValue) -> str | bool | None:
    match value:
        case SelectValue():
            return value.value
        case ToggleValue():
            return value.value
        case _:
            return None


def is_visible(definition: InputDefinition, inputs: dict[str, InputValue]) -> bool:
    condition = definition.visible_when
    if condition is None:
        return True
    controller = inputs.get(condition.input)
    return controller is not None and selected_value(controller) in condition.values


def validate_value(definition: InputDefinition, value: InputValue) -> None:
    match definition, value:
        case TextInput(), TextValue():
            if len(value.value) > definition.max_length:
                raise ValueError(f"Input {definition.key} is too long")
            if definition.required and not value.value:
                raise ValueError(f"Input {definition.key} is required")
        case SelectInput(), SelectValue():
            if value.value not in {option.value for option in definition.options}:
                raise ValueError(f"Input {definition.key} has an invalid selection")
        case AudioInput(), AudioValue():
            pass
        case ToggleInput(), ToggleValue():
            pass
        case NumberInput(), NumberValue():
            number = Fraction(value.value.numerator, value.value.denominator)
            minimum = Fraction(
                definition.minimum.numerator, definition.minimum.denominator
            )
            maximum = Fraction(
                definition.maximum.numerator, definition.maximum.denominator
            )
            if not minimum <= number <= maximum:
                raise ValueError(f"Input {definition.key} is outside its range")
        case TableInput(), TableValue():
            columns = {column.key: column for column in definition.columns}
            for row in value.rows:
                if unknown := row.keys() - columns.keys():
                    raise ValueError(
                        f"Input {definition.key} has unknown columns: {sorted(unknown)}"
                    )
                for key, column in columns.items():
                    cell = row.get(key, "")
                    if column.required and not cell:
                        raise ValueError(f"Table column {key} is required")
                    if len(cell) > column.max_length:
                        raise ValueError(f"Table column {key} is too long")
        case _:
            raise ValueError(f"Input {definition.key} has the wrong value type")


def validate_inputs(request: SynthesisRequest) -> None:
    definitions = {
        definition.key: definition for definition in model_info(request).inputs
    }
    active = {
        key
        for key, definition in definitions.items()
        if is_visible(definition, request.inputs)
    }
    if unknown := request.inputs.keys() - active:
        raise ValueError(f"Unknown or inactive inputs: {sorted(unknown)}")
    if missing := active - request.inputs.keys():
        raise ValueError(f"Missing inputs: {sorted(missing)}")
    for key, value in request.inputs.items():
        validate_value(definitions[key], value)


def typed_value(
    request: SynthesisRequest, key: str, expected: type[ValueModel]
) -> ValueModel:
    value = request.inputs[key]
    if not isinstance(value, expected):
        raise ValueError(f"Input {key} has the wrong value type")
    return value


def text(request: SynthesisRequest, key: str) -> str:
    return typed_value(request, key, TextValue).value


def selection(request: SynthesisRequest, key: str) -> str:
    return typed_value(request, key, SelectValue).value


def request_precision(request: SynthesisRequest) -> Precision:
    return PRECISION_VALIDATOR.validate_python(selection(request, "precision"))


def audio(request: SynthesisRequest, key: str) -> Audio:
    return typed_value(request, key, AudioValue).value


def toggle(request: SynthesisRequest, key: str) -> bool:
    return typed_value(request, key, ToggleValue).value


def number(request: SynthesisRequest, key: str) -> Fraction:
    value = typed_value(request, key, NumberValue).value
    return Fraction(value.numerator, value.denominator)


def integer(request: SynthesisRequest, key: str) -> int:
    value = number(request, key)
    if value.denominator != 1:
        raise ValueError(f"Input {key} must be an integer")
    return value.numerator


def parse_qwen(request: SynthesisRequest) -> ModelRequest:
    language = QWEN_LANGUAGE_VALIDATOR.validate_python(selection(request, "language"))
    match request.model:
        case (
            "Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice"
            | "Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice"
        ):
            instruction = (
                text(request, "instruction")
                if "instruction" in request.inputs
                else None
            )
            return QwenCustomVoiceRequest(
                model=request.model,
                text=text(request, "text"),
                language=language,
                speaker=QWEN_SPEAKER_VALIDATOR.validate_python(
                    selection(request, "speaker")
                ),
                instruction=instruction or None,
            )
        case "Qwen/Qwen3-TTS-12Hz-0.6B-Base" | "Qwen/Qwen3-TTS-12Hz-1.7B-Base":
            reference_text = text(request, "reference_text").strip() or None
            return QwenVoiceCloneRequest(
                model=request.model,
                text=text(request, "text"),
                language=language,
                reference_audio=audio(request, "reference_audio"),
                reference_text=reference_text,
                audio_only=reference_text is None,
            )
        case "Qwen/Qwen3-TTS-12Hz-1.7B-VoiceDesign":
            return QwenVoiceDesignRequest(
                model=request.model,
                text=text(request, "text"),
                language=language,
                instruction=text(request, "instruction"),
            )
        case _:
            raise ValueError(f"Model {request.model} is not a Qwen model")


def parse_index_emotion(request: SynthesisRequest) -> IndexEmotion:
    mode = selection(request, "emotion_mode")
    if mode == "speaker":
        return SpeakerEmotion()
    strength = number(request, "emotion_strength")
    if mode == "audio":
        return AudioEmotion(
            audio=audio(request, "emotion_audio"), strength=float(strength)
        )
    randomize = toggle(request, "randomize_emotion")
    if mode == "text":
        return TextEmotion(
            text=text(request, "emotion_text") or None,
            strength=float(strength),
            randomize=randomize,
        )
    if mode != "factors":
        raise ValueError(f"Unknown IndexTTS 2 emotion mode {mode}")
    factor_values = [
        number(request, f"emotion_{name}")
        for name in (
            "happy",
            "angry",
            "sad",
            "afraid",
            "disgusted",
            "melancholic",
            "surprised",
            "calm",
        )
    ]
    return FactorEmotion(
        factors=EmotionFactors(
            happy=float(factor_values[0]),
            angry=float(factor_values[1]),
            sad=float(factor_values[2]),
            afraid=float(factor_values[3]),
            disgusted=float(factor_values[4]),
            melancholic=float(factor_values[5]),
            surprised=float(factor_values[6]),
            calm=float(factor_values[7]),
        ),
        strength=float(strength),
        randomize=randomize,
    )


def parse_index(request: SynthesisRequest) -> IndexTts2Request:
    model = INDEX_MODEL_VALIDATOR.validate_python(request.model)
    timing_mode = selection(request, "timing_mode")
    timing: IndexTiming
    if timing_mode == "speed_factor":
        timing = SpeedTiming(factor=number(request, "speed_factor"))
    elif timing_mode == "duration":
        timing = DurationTiming(seconds=number(request, "duration"))
    else:
        raise ValueError(f"Unknown IndexTTS 2 timing mode {timing_mode}")
    glossary_value = typed_value(request, "glossary", TableValue)
    do_sample = toggle(request, "do_sample")
    typical_sampling = toggle(request, "typical_sampling") if do_sample else False
    typical_mass = float(number(request, "typical_mass")) if typical_sampling else 0.9
    return IndexTts2Request(
        model=model,
        text=text(request, "text"),
        language=(
            INDEX_LANGUAGE_VALIDATOR.validate_python(selection(request, "language"))
            if model == "IndexTeam/IndexTTS-2.5"
            else None
        ),
        voice=audio(request, "reference_audio"),
        emotion=parse_index_emotion(request),
        timing=timing,
        sampling=IndexSampling(
            do_sample=do_sample,
            typical_sampling=typical_sampling,
            typical_mass=typical_mass,
            top_p=float(number(request, "top_p")),
            top_k=integer(request, "top_k"),
            temperature=float(number(request, "temperature")),
            length_penalty=float(number(request, "length_penalty")),
            num_beams=integer(request, "num_beams"),
            repetition_penalty=float(number(request, "repetition_penalty")),
            max_mel_tokens=integer(request, "max_mel_tokens"),
        ),
        max_text_tokens_per_segment=integer(request, "max_text_tokens_per_segment"),
        intersegment_silence=number(request, "intersegment_silence"),
        glossary=[
            GlossaryEntry(
                term=row["term"],
                chinese=row.get("chinese") or None,
                english=row.get("english") or None,
            )
            for row in glossary_value.rows
        ],
    )


def parse_request(request: SynthesisRequest) -> ModelRequest:
    validate_inputs(request)
    if request.model in ("IndexTeam/IndexTTS-2", "IndexTeam/IndexTTS-2.5"):
        return parse_index(request)
    return parse_qwen(request)


def request_text(request: SynthesisRequest) -> str:
    value = request.inputs.get("text")
    return value.value if isinstance(value, TextValue) else ""
