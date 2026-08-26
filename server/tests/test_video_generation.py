from fractions import Fraction
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import TestCase
from unittest.mock import MagicMock, patch

import torch
from safetensors import safe_open
from safetensors.torch import save_file

from api.video_generation.minimax_h3.config import (
    ReferenceSpec,
    align_num_frames,
    validate_canvas,
    validate_references,
)
from api.video_generation.minimax_h3.inference import (
    GenerationRequest as H3Request,
    _load_conditioning_checkpoint,
    _save_conditioning_checkpoint,
    generate,
)
from api.video_generation.minimax_h3.lora import convert_musubi_lora
from api.video_generation.protocol import (
    GenerationRequest,
    InputValue,
    Media,
    MediaValue,
    NumberValue,
    Rational,
    SelectValue,
    TextValue,
)
from api.video_generation.requests import parse_request


def rational(value: int) -> NumberValue:
    return NumberValue(kind="number", value=Rational(numerator=value, denominator=1))


def base_request(**extra: InputValue) -> GenerationRequest:
    inputs: dict[str, InputValue] = {
        "workflow": SelectValue(kind="select", value="t2va"),
        "prompt": TextValue(kind="text", value="A fox walks through snow."),
        "resolution": SelectValue(kind="select", value="768x768"),
        "duration": rational(5),
        "steps": rational(30),
        "seed": rational(42),
        "attention": SelectValue(kind="select", value="default"),
        "quantization": SelectValue(kind="select", value="bf16"),
        "memory": SelectValue(kind="select", value="normal"),
    }
    inputs.update(extra)
    return GenerationRequest(model="MiniMaxAI/MiniMax-H3", inputs=inputs)


class VideoGenerationChecks(TestCase):
    def test_request_and_h3_constraints(self) -> None:
        parsed = parse_request(base_request())
        self.assertEqual(align_num_frames(parsed.duration), 124)
        self.assertEqual((parsed.width, parsed.height), (768, 768))
        self.assertEqual(align_num_frames(Fraction(5)), 124)
        validate_canvas(768, 768)
        with self.assertRaisesRegex(ValueError, "multiples of 32"):
            validate_canvas(769, 768)

    def test_canvas_references_and_keyframes(self) -> None:
        with TemporaryDirectory() as temporary:
            source = Path(temporary) / "ref.wav"
            source.touch()
            with self.assertRaisesRegex(ValueError, "only reference modality"):
                validate_references([ReferenceSpec("audio", str(source))])
            request = base_request(
                workflow=SelectValue(kind="select", value="fl2va"),
                first_frame=MediaValue(kind="media", items=[]),
                last_frame=MediaValue(kind="media", items=[]),
            )
            with self.assertRaisesRegex(ValueError, "requires a first frame"):
                parse_request(request)

    def test_media_is_staged_under_a_sanitized_content_hash(self) -> None:
        request = base_request(
            workflow=SelectValue(kind="select", value="fl2va"),
            first_frame=MediaValue(
                kind="media",
                items=[
                    Media(
                        kind="image",
                        filename="../../unsafe image.PNG",
                        data=b"image bytes",
                    )
                ],
            ),
            last_frame=MediaValue(kind="media", items=[]),
        )
        with TemporaryDirectory() as temporary, patch.dict(
            "os.environ", {"SHRIMPLY_VIDEO_GENERATION_CACHE": temporary}
        ):
            parsed = parse_request(request)
            staged = Path(parsed.image or "")
            self.assertTrue(staged.is_file())
            self.assertEqual(staged.parent.parent, Path(temporary))
            self.assertRegex(staged.name, r"^first_frame-0-[0-9a-f]{64}\.png$")

    def test_conditioning_checkpoint_round_trip(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = H3Request(
                "fl2va", "animate", root / "out.mp4", image="image.png"
            )
            checkpoint = root / "condition.safetensors"
            values = {
                "prompt_embeds": torch.randn(1, 3, 4),
                "text_token_tags": torch.tensor([1, 2, 3]),
                "condition_latents": [torch.randn(1, 2, 1, 3, 4)],
                "height": 768,
                "width": 576,
                "num_frames": 124,
                "keyframe_anchors": ("start",),
            }
            _save_conditioning_checkpoint(checkpoint, request, values)
            restored = _load_conditioning_checkpoint(checkpoint, request)
            self.assertEqual(restored["height"], 768)
            self.assertEqual(restored["keyframe_anchors"], ("start",))
            self.assertTrue(
                torch.equal(restored["condition_latents"][0], values["condition_latents"][0])
            )

    def test_generation_translates_and_saves_latents(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            pipe = MagicMock()
            pipe._blocks.sub_blocks = {"decode": MagicMock()}
            pipe.return_value = {
                "latents": torch.zeros(1, 24, 2, 2, 2),
                "audio_latents": torch.zeros(2, 32, 4),
            }
            request = H3Request("t2va", "a fox", root / "out.mp4")
            with (
                patch(
                    "api.video_generation.minimax_h3.inference.build_pipeline",
                    return_value=(pipe, "default", "bf16"),
                ),
                patch(
                    "api.video_generation.minimax_h3.inference._release_generation_pipeline"
                ),
                patch(
                    "api.video_generation.minimax_h3.inference.decode_checkpoint",
                    return_value={"output": str(request.output), "sampling_rate": 32_000},
                ) as decode,
            ):
                result = generate(request)
                checkpoint_only = H3Request(
                    "t2va", "a fox", root / "checkpoint-only.mp4"
                )
                checkpoint_result = generate(checkpoint_only, decode_output=False)
            self.assertEqual(pipe.call_args.kwargs["num_frames"], 124)
            self.assertTrue((root / "out.mp4.latents.safetensors").is_file())
            self.assertTrue(
                (root / "checkpoint-only.mp4.latents.safetensors").is_file()
            )
            self.assertEqual(result["sampling_rate"], 32_000)
            self.assertEqual(checkpoint_result["frames"], 124)
            decode.assert_called_once()

    def test_musubi_fused_qkv_conversion(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "musubi.safetensors"
            tensors = {}
            for block in range(50):
                for module, output in {
                    "attn_qkv_proj": 9,
                    "attn_out_proj": 5,
                    "mlp_fc1": 8,
                    "mlp_fc2": 5,
                }.items():
                    prefix = f"lora_unet_blocks_{block}_{module}"
                    tensors[f"{prefix}.alpha"] = torch.tensor(2.0)
                    tensors[f"{prefix}.lora_down.weight"] = torch.ones(2, 4)
                    tensors[f"{prefix}.lora_up.weight"] = (
                        torch.arange(output * 2).reshape(output, 2).float()
                    )
            save_file(
                tensors,
                source,
                metadata={
                    "modelspec.architecture": "MiniMax-H3/lora",
                    "ss_h3_training_mode": "fl2va",
                },
            )
            output = convert_musubi_lora(source, root / "converted.safetensors")
            with safe_open(output, framework="pt", device="cpu") as handle:
                self.assertEqual(len(list(handle.keys())), 600)
                prefix = "transformer.transformer_blocks.0.attn."
                self.assertEqual(
                    handle.get_tensor(prefix + "to_q.lora_B.weight").shape,
                    (3, 2),
                )
                self.assertEqual(handle.metadata()["training_mode"], "fl2va")
