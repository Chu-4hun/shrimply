from manimlib import *


class QuickShowcase(Scene):
    """A short combined scene that exercises both 2D and expensive 3D paths."""

    def construct(self):
        title = Text("SHRIMPLY / MANIM", font_size=54)
        title.set_color_by_gradient(BLUE, TEAL, YELLOW)
        subtitle = Text("vector  -  latex  -  surface", font_size=26)
        subtitle.next_to(title, DOWN)
        self.play(Write(title), FadeIn(subtitle, shift=UP), run_time=0.8)
        self.play(title.animate.to_edge(UP).scale(0.65), FadeOut(subtitle), run_time=0.6)

        equation = Tex(r"e^{i\pi}+1=0")
        equation.set_color_by_tex(r"\pi", YELLOW)
        integral = Tex(r"\int_{-\infty}^{\infty}e^{-x^2}\,dx=\sqrt{\pi}")
        integral.set_color_by_tex(r"\pi", TEAL)
        latex = VGroup(equation, integral).arrange(DOWN, buff=0.45)
        box = SurroundingRectangle(latex, color=BLUE, buff=0.3)
        self.play(Write(equation), run_time=0.7)
        self.play(TransformFromCopy(equation, integral), ShowCreation(box), run_time=0.8)
        self.play(latex.animate.scale(0.72).to_corner(UL), box.animate.scale(0.72).to_corner(UL))

        plane = NumberPlane(
            x_range=(-4, 4, 1),
            y_range=(-2, 2, 1),
            width=8,
            height=4,
        )
        plane.shift(DOWN * 0.8)
        wave = ParametricCurve(
            lambda t: plane.c2p(t, 0.8 * np.sin(2.5 * t)),
            t_range=(-4, 4, 0.04),
            color=YELLOW,
        )
        shapes = VGroup(Circle(), Square(), Triangle(), RegularPolygon(n=6))
        shapes.set_height(0.8)
        shapes.arrange(RIGHT, buff=0.55)
        shapes.to_edge(DOWN)
        shapes.set_color_by_gradient(RED, ORANGE, GREEN, BLUE)
        self.play(ShowCreation(plane), ShowCreation(wave), run_time=0.8)
        self.play(
            LaggedStart(*[GrowFromCenter(shape) for shape in shapes], lag_ratio=0.12),
            run_time=0.8,
        )

        tracker = ValueTracker(0)
        orbit = Circle(radius=1.1, color=TEAL).shift(RIGHT * 3 + UP * 0.5)
        moving_dot = Dot(color=YELLOW)
        moving_dot.add_updater(
            lambda dot: dot.move_to(orbit.point_from_proportion(tracker.get_value() % 1))
        )
        radius = always_redraw(lambda: Line(orbit.get_center(), moving_dot.get_center(), color=PINK))
        self.play(ShowCreation(orbit), FadeIn(moving_dot), ShowCreation(radius), run_time=0.5)
        self.play(
            tracker.animate.set_value(2),
            *[shape.animate.rotate(PI).scale(0.75) for shape in shapes],
            run_time=1.8,
            rate_func=linear,
        )
        moving_dot.clear_updaters()
        self.play(
            *[
                FadeOut(mob)
                for mob in [latex, box, plane, wave, shapes, orbit, moving_dot, radius]
            ],
            title.animate.set_opacity(0.35),
            run_time=0.7,
        )

        frame = self.camera.frame
        frame.set_euler_angles(theta=-35 * DEGREES, phi=68 * DEGREES)
        axes = ThreeDAxes()
        surface = ParametricSurface(
            lambda u, v: np.array(
                [
                    u,
                    v,
                    0.55 * np.sin(2.2 * u) * np.cos(2.2 * v),
                ]
            ),
            u_range=(-3.2, 3.2),
            v_range=(-2.4, 2.4),
            resolution=(72, 54),
        )
        surface.set_color_by_gradient(BLUE_E, TEAL, GREEN, YELLOW)
        surface.set_opacity(0.88)
        surface.set_shading(0.35, 0.45, 0.25)
        sphere = Sphere(radius=0.7, resolution=(48, 32))
        sphere.set_color(RED)
        sphere.set_shading(0.4, 0.5, 0.3)
        sphere.move_to(OUT * 1.4)
        self.camera.light_source.move_to(np.array([-4.0, -3.0, 6.0]))
        self.play(ShowCreation(axes), FadeIn(surface), GrowFromCenter(sphere), run_time=1.2)
        self.play(
            frame.animate.increment_theta(100 * DEGREES),
            sphere.animate.move_to(IN * 1.4).rotate(PI, axis=UP),
            run_time=3.2,
            rate_func=linear,
        )
        self.play(
            surface.animate.apply_function(
                lambda point: point + 0.18 * np.sin(2 * point[0]) * OUT
            ),
            frame.animate.increment_phi(-18 * DEGREES),
            run_time=1.5,
        )
        self.play(FadeOut(Group(axes, surface, sphere, title)), run_time=0.7)


class LatexShowcase(Scene):
    def construct(self):
        heading = Text("LaTeX and 2D transforms", font_size=42).to_edge(UP)
        equations = VGroup(
            Tex(r"\nabla\cdot\vec E=\frac{\rho}{\varepsilon_0}"),
            Tex(r"R_{\mu\nu}-\frac12 Rg_{\mu\nu}=8\pi T_{\mu\nu}"),
            Tex(r"\mathcal F\{f*g\}=\mathcal F\{f\}\mathcal F\{g\}"),
        ).arrange(DOWN, buff=0.55)
        equations.set_color_by_gradient(BLUE, TEAL, YELLOW)
        self.play(Write(heading), run_time=0.5)
        self.play(
            LaggedStart(*[Write(equation) for equation in equations], lag_ratio=0.25),
            run_time=1.8,
        )
        boxes = VGroup(*[SurroundingRectangle(eq, buff=0.18) for eq in equations])
        boxes.set_color_by_gradient(PINK, ORANGE, GREEN)
        self.play(LaggedStart(*[ShowCreation(box) for box in boxes], lag_ratio=0.15))
        self.play(
            equations.animate.arrange(RIGHT, buff=0.35).scale(0.58),
            FadeOut(boxes),
            run_time=1.2,
        )
        self.play(FadeOut(VGroup(heading, equations)), run_time=0.5)


class GeometryShowcase(Scene):
    def construct(self):
        plane = NumberPlane()
        circles = VGroup(
            *[
                Circle(radius=0.35 + 0.11 * index)
                .set_color(interpolate_color(BLUE, YELLOW, index / 11))
                .rotate(index * PI / 12)
                for index in range(12)
            ]
        )
        polygons = VGroup(*[RegularPolygon(n=sides) for sides in range(3, 10)])
        polygons.set_height(1.0)
        polygons.arrange(RIGHT, buff=0.25).to_edge(DOWN)
        polygons.set_color_by_gradient(RED, ORANGE, GREEN, BLUE)
        self.play(ShowCreation(plane), run_time=0.5)
        self.play(LaggedStart(*[ShowCreation(circle) for circle in circles], lag_ratio=0.05))
        self.play(
            LaggedStart(*[GrowFromCenter(polygon) for polygon in polygons], lag_ratio=0.08)
        )
        self.play(
            circles.animate.rotate(TAU).scale(1.6),
            *[polygon.animate.rotate(PI) for polygon in polygons],
            run_time=2,
            rate_func=linear,
        )
        self.play(FadeOut(VGroup(plane, circles, polygons)), run_time=0.5)


class GpuSurfaceShowcase(Scene):
    """Intentionally dense surfaces for profiling the expensive 3D render path."""

    def construct(self):
        frame = self.camera.frame
        frame.set_euler_angles(theta=-45 * DEGREES, phi=65 * DEGREES)
        axes = ThreeDAxes()
        wave = ParametricSurface(
            lambda u, v: np.array(
                [u, v, 0.65 * np.sin(2.5 * u + v) * np.cos(2 * v - u)]
            ),
            u_range=(-3.5, 3.5),
            v_range=(-3.0, 3.0),
            resolution=(96, 80),
        )
        wave.set_color_by_gradient(BLUE_E, BLUE, TEAL, GREEN, YELLOW)
        wave.set_opacity(0.9)
        wave.set_shading(0.4, 0.5, 0.25)
        sphere = Sphere(radius=1.0, resolution=(72, 48)).shift(OUT * 1.8)
        sphere.set_color(RED)
        sphere.set_shading(0.5, 0.45, 0.3)
        self.camera.light_source.move_to(np.array([-5.0, -4.0, 7.0]))
        self.play(ShowCreation(axes), FadeIn(wave), FadeIn(sphere), run_time=1.2)
        self.play(
            frame.animate.increment_theta(TAU),
            sphere.animate.rotate(2 * TAU, axis=normalize(UP + RIGHT)),
            run_time=5,
            rate_func=linear,
        )
        self.play(
            wave.animate.apply_function(
                lambda point: point
                + 0.22 * np.sin(3 * point[0] - 2 * point[1]) * OUT
            ),
            sphere.animate.move_to(IN * 1.8),
            frame.animate.increment_phi(-20 * DEGREES),
            run_time=2,
        )
        self.play(FadeOut(Group(axes, wave, sphere)), run_time=0.6)
