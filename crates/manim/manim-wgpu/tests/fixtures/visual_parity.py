import os
from pathlib import Path

import numpy as np

from manimlib import (
    Arrow,
    BLUE,
    Circle,
    DotCloud,
    FadeIn,
    GREEN,
    GREY_E,
    GrowArrow,
    ImageMobject,
    IN,
    LEFT,
    ORANGE,
    ORIGIN,
    PI,
    Rotate,
    RIGHT,
    RoundedRectangle,
    Scene,
    Sphere,
    Square,
    Tex,
    Text,
    TexturedSurface,
    ThreeDScene,
    Transform,
    UP,
    VGroup,
    WHITE,
)
from manimlib.renderer.uniform_block import COMMON_UNIFORMS, uniform_block_dtype


class WeightedImage(ImageMobject):
    shader_file = str(Path(__file__).with_name("weighted_image.wgsl"))
    uniform_dtype = uniform_block_dtype(*COMMON_UNIFORMS, ("custom_weights", 2))


class VisualParity(Scene):
    def construct(self) -> None:
        boxes = VGroup(
            self.box("I", GREY_E),
            self.box("P", GREEN),
            self.box("B", BLUE),
        ).arrange(RIGHT, buff=0.6)
        arrow = Arrow(
            boxes[0].get_top(),
            boxes[1].get_top(),
            path_arc=-0.8,
            color=WHITE,
            stroke_width=5,
        )
        arrow.shift(0.1 * UP)
        image = WeightedImage(os.environ["SHRIMPLY_MANIM_CHECKER"])
        image.set_uniform(custom_weights=np.array([1, 0], dtype=np.float32))
        image.set_height(0.6).to_edge(UP)
        self.add(image)

        self.play(FadeIn(boxes), run_time=0.5)
        self.wait(0.5)
        self.play(GrowArrow(arrow), run_time=0.5)
        self.wait(0.5)

    @staticmethod
    def box(label: str, color: str) -> VGroup:
        rectangle = RoundedRectangle(
            width=1.5,
            height=1.5,
            corner_radius=0.2,
            fill_color=color,
            fill_opacity=1,
            stroke_width=0,
        )
        if label == "P":
            rectangle.set_color_by_code("color = vec4f(color.gbr, color.a);")
        text = Text(label, font_size=48, color=WHITE)
        return VGroup(rectangle, text).move_to(ORIGIN)


class VectorMorphParity(Scene):
    def construct(self) -> None:
        source = Circle(radius=1.1, color=BLUE, fill_opacity=0.8)
        source.shift(1.5 * LEFT)
        target = Square(side_length=1.8, color=GREEN, fill_opacity=1)
        target.shift(1.5 * RIGHT).rotate(PI / 4)
        self.add(source)
        self.play(Transform(source, target, path_arc=PI / 3), run_time=1)
        self.wait(0.5)


class LatexParity(Scene):
    def construct(self) -> None:
        equation = Tex(R"e^{\pi i} + 1 = 0", font_size=96)
        equation.set_color_by_tex(R"\pi", ORANGE)
        self.play(FadeIn(equation), run_time=0.5)
        self.play(equation.animate.shift(0.8 * UP).scale(0.8), run_time=0.5)
        self.wait(0.5)


class ThreeDParity(ThreeDScene):
    def construct(self) -> None:
        sphere = Sphere(radius=1.4, resolution=(12, 6))
        sphere.set_color(BLUE)
        sphere.shift(0.4 * IN)
        self.add(sphere)
        self.play(Rotate(sphere, PI / 2, axis=UP), run_time=1)
        self.wait(0.5)


class GenericPipelineParity(ThreeDScene):
    def construct(self) -> None:
        dots = DotCloud(
            np.array([[-1.8, -0.5, 0], [-1.3, 0.2, 0], [-0.8, -0.1, 0]]),
            color=ORANGE,
            radius=0.12,
            glow_factor=0.4,
        )
        sphere = Sphere(radius=0.9, resolution=(8, 4)).shift(1.0 * RIGHT)
        textured = TexturedSurface(
            sphere,
            os.environ["SHRIMPLY_MANIM_CHECKER"],
        )
        self.add(dots, textured)
        self.play(
            dots.animate.shift(0.4 * UP),
            Rotate(textured, PI / 3, axis=UP),
            run_time=0.5,
        )
        self.wait(0.5)
