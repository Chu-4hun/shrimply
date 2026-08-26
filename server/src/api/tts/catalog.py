from api.tts.protocol import (
    AudioInput,
    InputDefinition,
    ModelInfo,
    NumberInput,
    Rational,
    SelectInput,
    SelectOption,
    TableColumn,
    TableInput,
    TextInput,
    ToggleInput,
    VisibleWhen,
)


def ratio(numerator: int, denominator: int = 1) -> Rational:
    return Rational(numerator=numerator, denominator=denominator)


INDEX_MAX_MEL_TOKENS = 1_815
INDEX_MAX_ACOUSTIC_SEQUENCE = 8_192


QWEN_LANGUAGES = [
    SelectOption(value=name, label="Auto-detect" if name == "Auto" else name)
    for name in (
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
    )
]
QWEN_SPEAKERS = [
    SelectOption(value=name, label=name.replace("_", " "))
    for name in (
        "Vivian",
        "Serena",
        "Uncle_Fu",
        "Dylan",
        "Eric",
        "Ryan",
        "Aiden",
        "Ono_Anna",
        "Sohee",
    )
]
QWEN_TEXT = TextInput(
    key="text",
    label="Text",
    required=True,
    multiline=True,
    purpose="text",
)
PRECISION_INPUT = SelectInput(
    key="precision",
    label="Precision",
    options=[
        SelectOption(value="auto", label="Automatic"),
        SelectOption(value="bfloat16", label="BF16"),
        SelectOption(value="float16", label="FP16"),
        SelectOption(value="float32", label="FP32"),
    ],
    default="auto",
)
QWEN_LANGUAGE = SelectInput(
    key="language",
    label="Language",
    options=QWEN_LANGUAGES,
    default="Auto",
)
QWEN_SPEAKER = SelectInput(
    key="speaker",
    label="Speaker",
    options=QWEN_SPEAKERS,
    default="Vivian",
)
INDEX_LANGUAGE = SelectInput(
    key="language",
    label="Language",
    options=[
        SelectOption(value=code, label=label)
        for code, label in (
            ("zh", "Chinese"),
            ("en", "English"),
            ("ja", "Japanese"),
            ("es", "Spanish"),
            ("ar", "Arabic"),
        )
    ],
    default="en",
)


def custom_voice_inputs(instruction: bool) -> list[InputDefinition]:
    inputs: list[InputDefinition] = [
        PRECISION_INPUT,
        QWEN_TEXT,
        QWEN_LANGUAGE,
        QWEN_SPEAKER,
    ]
    if instruction:
        inputs.append(
            TextInput(
                key="instruction",
                label="Instruction",
                multiline=True,
                max_length=1024,
            )
        )
    return inputs


VOICE_CLONE_INPUTS: list[InputDefinition] = [
    PRECISION_INPUT,
    QWEN_TEXT,
    QWEN_LANGUAGE,
    AudioInput(key="reference_audio", label="Reference audio"),
    TextInput(
        key="reference_text",
        label="Reference transcript",
        multiline=True,
    ),
]
VOICE_DESIGN_INPUTS: list[InputDefinition] = [
    PRECISION_INPUT,
    QWEN_TEXT,
    QWEN_LANGUAGE,
    TextInput(
        key="instruction",
        label="Voice description",
        required=True,
        multiline=True,
        max_length=1024,
    ),
]

INDEX_EMOTION_MODE = SelectInput(
    key="emotion_mode",
    label="Emotion source",
    options=[
        SelectOption(value="speaker", label="Speaker audio"),
        SelectOption(value="audio", label="Emotion audio"),
        SelectOption(value="factors", label="Emotion factors"),
        SelectOption(value="text", label="Emotion text"),
    ],
    default="speaker",
)
INDEX_TIMING_MODE = SelectInput(
    key="timing_mode",
    label="Timing control",
    options=[
        SelectOption(
            value="speed_factor", label="Speed factor", purpose="speed_factor"
        ),
        SelectOption(value="duration", label="Duration", purpose="duration"),
    ],
    default="speed_factor",
)
INDEX_INPUTS: list[InputDefinition] = [
    TextInput(
        key="text",
        label="Text",
        required=True,
        multiline=True,
        max_length=16_384,
        purpose="text",
    ),
    AudioInput(key="reference_audio", label="Voice reference audio"),
    INDEX_EMOTION_MODE,
    AudioInput(
        key="emotion_audio",
        label="Emotion reference audio",
        visible_when=VisibleWhen(input="emotion_mode", values=["audio"]),
    ),
    TextInput(
        key="emotion_text",
        label="Emotion description",
        multiline=True,
        visible_when=VisibleWhen(input="emotion_mode", values=["text"]),
    ),
    *[
        NumberInput(
            key=f"emotion_{name}",
            label=label,
            default=ratio(0),
            minimum=ratio(0),
            maximum=ratio(4, 5),
            step=ratio(1, 100),
            presentation="number",
            visible_when=VisibleWhen(input="emotion_mode", values=["factors"]),
        )
        for name, label in (
            ("happy", "Happy"),
            ("angry", "Angry"),
            ("sad", "Sad"),
            ("afraid", "Afraid"),
            ("disgusted", "Disgusted"),
            ("melancholic", "Melancholic"),
            ("surprised", "Surprised"),
            ("calm", "Calm"),
        )
    ],
    NumberInput(
        key="emotion_strength",
        label="Emotion strength",
        default=ratio(1),
        minimum=ratio(0),
        maximum=ratio(1),
        step=ratio(1, 100),
        presentation="number",
        visible_when=VisibleWhen(
            input="emotion_mode", values=["audio", "factors", "text"]
        ),
    ),
    ToggleInput(
        key="randomize_emotion",
        label="Randomize emotion prototype",
        visible_when=VisibleWhen(input="emotion_mode", values=["factors", "text"]),
    ),
    INDEX_TIMING_MODE,
    NumberInput(
        key="speed_factor",
        label="Speed factor",
        default=ratio(1),
        minimum=ratio(1, INDEX_MAX_ACOUSTIC_SEQUENCE),
        maximum=ratio(INDEX_MAX_MEL_TOKENS),
        step=ratio(1, 100),
        purpose="speed_factor",
        visible_when=VisibleWhen(input="timing_mode", values=["speed_factor"]),
    ),
    NumberInput(
        key="duration",
        label="Duration in seconds",
        default=ratio(3),
        minimum=ratio(1, 100),
        maximum=ratio(600),
        step=ratio(1, 1000),
        purpose="duration",
        visible_when=VisibleWhen(input="timing_mode", values=["duration"]),
    ),
    ToggleInput(key="do_sample", label="Sample semantic tokens", default=True),
    ToggleInput(
        key="typical_sampling",
        label="Use typical sampling",
        visible_when=VisibleWhen(input="do_sample", values=[True]),
    ),
    NumberInput(
        key="typical_mass",
        label="Typical probability mass",
        default=ratio(9, 10),
        minimum=ratio(1, 100),
        maximum=ratio(99, 100),
        step=ratio(1, 100),
        presentation="number",
        visible_when=VisibleWhen(input="typical_sampling", values=[True]),
    ),
    NumberInput(
        key="top_p",
        label="Top-p",
        default=ratio(4, 5),
        minimum=ratio(1, 100),
        maximum=ratio(1),
        step=ratio(1, 100),
        presentation="number",
    ),
    NumberInput(
        key="top_k",
        label="Top-k",
        default=ratio(30),
        minimum=ratio(0),
        maximum=ratio(200),
        step=ratio(1),
    ),
    NumberInput(
        key="temperature",
        label="Temperature",
        default=ratio(4, 5),
        minimum=ratio(1, 100),
        maximum=ratio(2),
        step=ratio(1, 100),
        presentation="number",
    ),
    NumberInput(
        key="length_penalty",
        label="Length penalty",
        default=ratio(0),
        minimum=ratio(-10),
        maximum=ratio(10),
        step=ratio(1, 100),
    ),
    NumberInput(
        key="num_beams",
        label="Beams",
        default=ratio(3),
        minimum=ratio(1),
        maximum=ratio(20),
        step=ratio(1),
    ),
    NumberInput(
        key="repetition_penalty",
        label="Repetition penalty",
        default=ratio(10),
        minimum=ratio(1, 100),
        maximum=ratio(30),
        step=ratio(1, 100),
    ),
    NumberInput(
        key="max_mel_tokens",
        label="Maximum mel tokens",
        default=ratio(1500),
        minimum=ratio(1),
        maximum=ratio(INDEX_MAX_MEL_TOKENS),
        step=ratio(1),
    ),
    NumberInput(
        key="max_text_tokens_per_segment",
        label="Maximum text tokens per segment",
        default=ratio(120),
        minimum=ratio(1),
        maximum=ratio(600),
        step=ratio(1),
    ),
    NumberInput(
        key="intersegment_silence",
        label="Intersegment silence in seconds",
        default=ratio(1, 5),
        minimum=ratio(0),
        maximum=ratio(10),
        step=ratio(1, 1000),
    ),
    TableInput(
        key="glossary",
        label="Pronunciation glossary",
        columns=[
            TableColumn(key="term", label="Term", required=True, max_length=256),
            TableColumn(key="chinese", label="Chinese pronunciation"),
            TableColumn(key="english", label="English pronunciation"),
        ],
    ),
]

MODELS = [
    ModelInfo(
        id="Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice",
        label="Qwen3 TTS 0.6B Custom Voice",
        inputs=custom_voice_inputs(False),
    ),
    ModelInfo(
        id="Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice",
        label="Qwen3 TTS 1.7B Custom Voice",
        inputs=custom_voice_inputs(True),
    ),
    ModelInfo(
        id="Qwen/Qwen3-TTS-12Hz-0.6B-Base",
        label="Qwen3 TTS 0.6B Voice Clone",
        inputs=VOICE_CLONE_INPUTS,
    ),
    ModelInfo(
        id="Qwen/Qwen3-TTS-12Hz-1.7B-Base",
        label="Qwen3 TTS 1.7B Voice Clone",
        inputs=VOICE_CLONE_INPUTS,
    ),
    ModelInfo(
        id="Qwen/Qwen3-TTS-12Hz-1.7B-VoiceDesign",
        label="Qwen3 TTS 1.7B Voice Design",
        inputs=VOICE_DESIGN_INPUTS,
    ),
    ModelInfo(
        id="IndexTeam/IndexTTS-2",
        label="IndexTTS 2",
        inputs=[PRECISION_INPUT, *INDEX_INPUTS],
    ),
    ModelInfo(
        id="IndexTeam/IndexTTS-2.5",
        label="IndexTTS 2.5",
        inputs=[PRECISION_INPUT, INDEX_LANGUAGE, *INDEX_INPUTS],
    ),
]
