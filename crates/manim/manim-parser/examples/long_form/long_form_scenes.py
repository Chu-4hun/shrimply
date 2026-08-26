from manimlib import *

import math


LONG_SEGMENT_SECONDS = 25
DOT_CLOUD_POINTS = 512
LATTICE_COLUMNS = 12
LATTICE_ROWS = 7


def long_fourier_art():
    art = VGroup()
    colors = (BLUE, TEAL, GREEN, YELLOW, ORANGE, RED)
    for index, color in enumerate(colors, start=1):
        art.add(
            Circle(radius=0.42 * index).set_stroke(
                color,
                width=2.5,
                opacity=0.85,
            )
        )
    art.add(Line(LEFT * 3.2, RIGHT * 3.2).set_stroke(WHITE, width=2))
    return art


class FourierCheckpointMarathon(Scene):
    def construct(self):
        self.add(long_fourier_art())
        frame = self.camera.frame
        views = (
            (8, 0.4, 7.4),
            (-8, -0.4, 8.2),
            (12, 0.2, 7.8),
            (-12, -0.2, 8.4),
            (16, 0.5, 7.2),
            (-16, -0.5, 8.0),
            (20, 0.3, 7.6),
            (-20, -0.3, 8.3),
            (24, 0.4, 7.3),
            (-24, -0.4, 8.1),
            (0, 0.0, 8.0),
        )
        for gamma, center_x, height in views:
            self.play(
                frame.animate.reorient(
                    0,
                    0,
                    gamma,
                    center=(center_x, 0, 0),
                    height=height,
                ),
                run_time=LONG_SEGMENT_SECONDS,
                rate_func=linear,
            )


def long_point_art():
    indices = np.arange(DOT_CLOUD_POINTS, dtype=float)
    angle = indices * PI * (3.0 - math.sqrt(5.0))
    radius = 3.0 * np.sqrt((indices + 0.5) / DOT_CLOUD_POINTS)
    points = np.column_stack((radius * np.cos(angle), radius * np.sin(angle), np.zeros_like(radius)))
    return DotCloud(points, color=TEAL, radius=0.028, glow_factor=0.2)


class ChaoticVectorFieldMarathon(Scene):
    def construct(self):
        self.add(long_point_art())
        frame = self.camera.frame
        views = (
            (7, 0.3, 7.5),
            (-7, -0.3, 8.2),
            (11, 0.4, 7.2),
            (-11, -0.4, 8.4),
            (15, 0.2, 7.7),
            (-15, -0.2, 8.1),
            (19, 0.5, 7.3),
            (-19, -0.5, 8.3),
            (23, 0.3, 7.6),
            (0, 0.0, 8.0),
        )
        for gamma, center_x, height in views:
            self.play(
                frame.animate.reorient(
                    0,
                    0,
                    gamma,
                    center=(center_x, 0, 0),
                    height=height,
                ),
                run_time=LONG_SEGMENT_SECONDS,
                rate_func=linear,
            )


def long_lattice_art():
    cells = VGroup()
    palette = color_gradient([BLUE, TEAL, GREEN, YELLOW, ORANGE, RED], LATTICE_ROWS)
    for row in range(LATTICE_ROWS):
        for column in range(LATTICE_COLUMNS):
            phase = row * 0.47 + column * 0.31
            cell = Square(side_length=0.46 + 0.08 * math.sin(phase) ** 2)
            cell.move_to([
                (column - (LATTICE_COLUMNS - 1) / 2) * 0.62,
                (row - (LATTICE_ROWS - 1) / 2) * 0.62,
                0.0,
            ])
            cell.rotate(0.16 * math.sin(phase))
            cell.set_fill(palette[row], opacity=0.18)
            cell.set_stroke(palette[row], width=1.5, opacity=0.9)
            cells.add(cell)
    return cells


class MatrixLatticeMarathon(Scene):
    def construct(self):
        self.add(long_lattice_art())
        frame = self.camera.frame
        views = (
            (-6, -0.3, 8.4),
            (6, 0.3, 7.6),
            (-10, 0.2, 8.2),
            (10, -0.2, 7.8),
            (-14, -0.4, 8.3),
            (14, 0.4, 7.7),
            (-18, 0.3, 8.1),
            (18, -0.3, 7.9),
            (-22, 0.2, 8.2),
            (0, 0.0, 8.0),
        )
        for gamma, center_x, height in views:
            self.play(
                frame.animate.reorient(
                    0,
                    0,
                    gamma,
                    center=(center_x, 0, 0),
                    height=height,
                ),
                run_time=LONG_SEGMENT_SECONDS,
                rate_func=linear,
            )
