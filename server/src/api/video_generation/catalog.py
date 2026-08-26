from api.video_generation.protocol import (
    InputDefinition,
    MediaInput,
    ModelInfo,
    ModelOutput,
    NumberInput,
    Rational,
    SelectInput,
    SelectOption,
    TextInput,
    VisibleWhen,
)


def ratio(numerator: int, denominator: int = 1) -> Rational:
    return Rational(numerator=numerator, denominator=denominator)


WORKFLOW = SelectInput(
    key="workflow",
    label="Workflow",
    options=[
        SelectOption(value="t2va", label="Text to audio-video"),
        SelectOption(value="fl2va", label="First/last frame to audio-video"),
        SelectOption(value="ref2va", label="References to audio-video"),
    ],
    default="t2va",
)
PROMPT = TextInput(
    key="prompt",
    label="Prompt",
    required=True,
    multiline=True,
    max_length=16_384,
)
RESOLUTION = SelectInput(
    key="resolution",
    label="Resolution",
    options=[
        SelectOption(value="768x768", label="Square · 768×768"),
        SelectOption(value="960x544", label="Landscape · 960×544"),
        SelectOption(value="576x768", label="Portrait · 576×768"),
        SelectOption(value="custom", label="Custom"),
    ],
    default="768x768",
)
CUSTOM_WIDTH = NumberInput(
    key="width",
    label="Width",
    default=ratio(768),
    minimum=ratio(32),
    maximum=ratio(2048),
    step=ratio(32),
    visible_when=VisibleWhen(input="resolution", values=["custom"]),
)
CUSTOM_HEIGHT = NumberInput(
    key="height",
    label="Height",
    default=ratio(768),
    minimum=ratio(32),
    maximum=ratio(2048),
    step=ratio(32),
    visible_when=VisibleWhen(input="resolution", values=["custom"]),
)
DURATION = NumberInput(
    key="duration",
    label="Requested duration in seconds",
    default=ratio(5),
    minimum=ratio(5),
    maximum=ratio(15),
    step=ratio(1),
)
STEPS = NumberInput(
    key="steps",
    label="Scheduler steps",
    default=ratio(30),
    minimum=ratio(2),
    maximum=ratio(100),
    step=ratio(1),
)
SEED = NumberInput(
    key="seed",
    label="Seed",
    default=ratio(42),
    minimum=ratio(0),
    maximum=ratio(4_294_967_295),
    step=ratio(1),
)
ATTENTION = SelectInput(
    key="attention",
    label="Attention",
    options=[
        SelectOption(value="auto", label="Automatic"),
        SelectOption(value="default", label="Default"),
        SelectOption(value="flash3", label="Flash Attention 3"),
    ],
    default="auto",
)
QUANTIZATION = SelectInput(
    key="quantization",
    label="Quantization",
    options=[
        SelectOption(value="bf16", label="None (BF16)"),
        SelectOption(value="int8", label="INT8 weight-only"),
    ],
    default="bf16",
)
MEMORY = SelectInput(
    key="memory",
    label="Memory mode",
    options=[
        SelectOption(value="normal", label="Normal"),
        SelectOption(value="low_vram", label="Low VRAM"),
    ],
    default="normal",
    visible_when=VisibleWhen(input="quantization", values=["bf16"]),
)
FIRST_FRAME = MediaInput(
    key="first_frame",
    label="First frame",
    accepted=["image"],
    minimum_items=0,
    maximum_items=1,
    visible_when=VisibleWhen(input="workflow", values=["fl2va"]),
)
LAST_FRAME = MediaInput(
    key="last_frame",
    label="Last frame",
    accepted=["image"],
    minimum_items=0,
    maximum_items=1,
    visible_when=VisibleWhen(input="workflow", values=["fl2va"]),
)
REFERENCES = MediaInput(
    key="references",
    label="Ordered references",
    accepted=["image", "video", "audio"],
    minimum_items=1,
    maximum_items=12,
    ordered=True,
    visible_when=VisibleWhen(input="workflow", values=["ref2va"]),
)
LORA_SCALE = NumberInput(
    key="lora_scale",
    label="Sketch LoRA strength",
    default=ratio(1),
    minimum=ratio(0),
    maximum=ratio(4),
    step=ratio(1, 100),
    presentation="slider",
)

WAN_NEGATIVE_PROMPT = TextInput(
    key="negative_prompt",
    label="Negative prompt",
    default=(
        "Bright tones, overexposed, static, blurred details, subtitles, style, "
        "works, paintings, images, overall gray, worst quality, low quality, "
        "JPEG compression residue, ugly, incomplete, extra fingers, poorly drawn "
        "hands, poorly drawn faces, deformed, disfigured, misshapen limbs, fused "
        "fingers, still picture, messy background, three legs, many people in the "
        "background, walking backwards"
    ),
    multiline=True,
    max_length=16_384,
)
WAN_STEPS = NumberInput(
    key="steps",
    label="Scheduler steps",
    default=ratio(50),
    minimum=ratio(2),
    maximum=ratio(100),
    step=ratio(1),
)
WAN_GUIDANCE = NumberInput(
    key="guidance_scale",
    label="Guidance scale",
    default=ratio(5),
    minimum=ratio(1),
    maximum=ratio(10),
    step=ratio(1, 10),
    presentation="slider",
)
WAN21_RESOLUTION = SelectInput(
    key="resolution",
    label="Resolution",
    options=[
        SelectOption(value="832x480", label="Landscape · 832×480"),
        SelectOption(value="480x832", label="Portrait · 480×832"),
    ],
    default="832x480",
)
WAN22_WORKFLOW = SelectInput(
    key="workflow",
    label="Workflow",
    options=[
        SelectOption(value="t2v", label="Text to video"),
        SelectOption(value="i2v", label="Image to video"),
    ],
    default="t2v",
)
WAN22_RESOLUTION = SelectInput(
    key="resolution",
    label="Resolution",
    options=[
        SelectOption(value="1280x704", label="Landscape · 1280×704"),
        SelectOption(value="704x1280", label="Portrait · 704×1280"),
    ],
    default="1280x704",
)
WAN_FIRST_FRAME = MediaInput(
    key="first_frame",
    label="First frame",
    accepted=["image"],
    minimum_items=1,
    maximum_items=1,
    visible_when=VisibleWhen(input="workflow", values=["i2v"]),
)


def common_inputs(workflow: SelectInput = WORKFLOW) -> list[InputDefinition]:
    return [
        workflow,
        PROMPT,
        RESOLUTION,
        CUSTOM_WIDTH,
        CUSTOM_HEIGHT,
        DURATION,
        STEPS,
        SEED,
        ATTENTION,
        QUANTIZATION,
        MEMORY,
        FIRST_FRAME,
        LAST_FRAME,
    ]


SKETCH_WORKFLOW = SelectInput(
    key="workflow",
    label="Workflow",
    options=WORKFLOW.options[:2],
    default="t2va",
)

MODELS = [
    ModelInfo(
        id="MiniMaxAI/MiniMax-H3",
        label="MiniMax H3 Base",
        inputs=[*common_inputs(), REFERENCES],
        output=ModelOutput(video=True, audio=True),
    ),
    ModelInfo(
        id="Inner-Reflections/MiniMax-H3-Looping-Sketch-Anime",
        label="MiniMax H3 Looping Sketch Anime",
        inputs=[*common_inputs(SKETCH_WORKFLOW), LORA_SCALE],
        output=ModelOutput(video=True, audio=True),
    ),
    ModelInfo(
        id="Wan-AI/Wan2.1-T2V-1.3B-Diffusers",
        label="Wan 2.1 T2V 1.3B",
        inputs=[
            PROMPT,
            WAN_NEGATIVE_PROMPT,
            WAN21_RESOLUTION,
            WAN_STEPS,
            WAN_GUIDANCE,
            SEED,
        ],
        output=ModelOutput(video=True, audio=False),
    ),
    ModelInfo(
        id="Wan-AI/Wan2.2-TI2V-5B-Diffusers",
        label="Wan 2.2 TI2V 5B",
        inputs=[
            WAN22_WORKFLOW,
            PROMPT,
            WAN_NEGATIVE_PROMPT,
            WAN22_RESOLUTION,
            WAN_STEPS,
            WAN_GUIDANCE,
            SEED,
            WAN_FIRST_FRAME,
        ],
        output=ModelOutput(video=True, audio=False),
    ),
]
