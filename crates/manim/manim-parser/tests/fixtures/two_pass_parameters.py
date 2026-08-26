from fractions import Fraction

from manimlib import Scene, Square
from reflected_import import IMPORTED_SCALE
from shrimply_manim import use_fraction


class TwoPassParameters(Scene):
    def construct(self) -> None:
        hold = use_fraction(
            Fraction(1, 10),
            key="construct-hold",
            label="Construct hold",
        )
        self.add(Square().scale(IMPORTED_SCALE))
        self.wait(hold)
