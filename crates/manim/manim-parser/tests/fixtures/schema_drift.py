from manimlib import Scene
from shrimply_manim import use_bool, use_int


MODE = use_bool(False, key="mode", label="Mode")
if MODE:
    EXTRA = use_int(1, key="extra", label="Extra")


class SchemaDrift(Scene):
    def construct(self) -> None:
        self.wait(0.1)
