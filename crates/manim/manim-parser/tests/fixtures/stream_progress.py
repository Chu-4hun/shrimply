from fractions import Fraction
from time import sleep

from manimlib import Scene


class StreamProgress(Scene):
    def construct(self) -> None:
        for _ in range(3):
            self.wait(Fraction(1, 10))
            sleep(0.2)
