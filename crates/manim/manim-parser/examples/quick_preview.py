from manimlib import *


GRID_SECONDS = 1.6
FORMULA_SECONDS = 1.3
PERSPECTIVE_SECONDS = 2.0
SURFACE_SECONDS = 2.2
ORBIT_SECONDS = 3.0
SLICE_SECONDS = 1.8
TRACE_SECONDS = 2.8
OUTRO_SECONDS = 1.2
HOLD_SECONDS = 0.5
SURFACE_AMPLITUDE = 0.75


class QuickPreview(ThreeDScene):
    def construct(self):
        self.frame.reorient(0, 0)

        plane = NumberPlane(
            x_range=(-6, 6, 1),
            y_range=(-3, 3, 1),
            width=12,
            height=6,
            background_line_style={
                "stroke_color": BLUE_D,
                "stroke_width": 2,
                "stroke_opacity": 0.65,
            },
            faded_line_style={
                "stroke_color": BLUE_E,
                "stroke_width": 1,
                "stroke_opacity": 0.25,
            },
        )
        plane.add_coordinate_labels(font_size=18)

        title = Text("FROM GRID TO SURFACE", font_size=36).to_edge(UP)
        equation = Tex(
            R"z=f(x,y)=\frac{3}{4}\sin(x)\cos(y)",
            font_size=42,
        ).next_to(title, DOWN)
        title.fix_in_frame()
        equation.fix_in_frame()

        self.play(
            ShowCreation(plane),
            FadeIn(title, shift=DOWN * 0.2),
            run_time=GRID_SECONDS,
        )
        self.play(FadeIn(equation, shift=UP * 0.15), run_time=FORMULA_SECONDS)
        self.wait(HOLD_SECONDS)

        axes = ThreeDAxes(
            x_range=(-4, 4, 1),
            y_range=(-3, 3, 1),
            z_range=(-2, 2, 1),
            width=10,
            height=6,
            depth=4,
            axis_config={"stroke_color": GREY_B, "stroke_width": 2},
        )
        axes.add_axis_labels(font_size=24)
        self.play(
            FadeOut(plane),
            ShowCreation(axes),
            self.frame.animate.reorient(-35, 68, 0, height=8),
            run_time=PERSPECTIVE_SECONDS,
        )

        surface = axes.get_graph(
            lambda x, y: SURFACE_AMPLITUDE * np.sin(x) * np.cos(y),
            u_range=(-PI, PI),
            v_range=(-PI, PI),
            resolution=(41, 41),
            color=BLUE_D,
            opacity=0.72,
        )
        surface.set_color_by_gradient(BLUE_E, TEAL, GREEN)
        mesh = SurfaceMesh(
            surface,
            resolution=(17, 17),
            stroke_color=WHITE,
            stroke_width=1,
        )
        self.play(
            FadeIn(surface),
            ShowCreation(mesh, lag_ratio=0.02),
            run_time=SURFACE_SECONDS,
        )
        self.wait(HOLD_SECONDS)

        gradient = Tex(
            R"\nabla f=\frac{3}{4}"
            R"\big(\cos(x)\cos(y),-\sin(x)\sin(y)\big)",
            font_size=36,
        ).move_to(equation)
        gradient.fix_in_frame()
        self.play(
            Transform(equation, gradient),
            self.frame.animate.increment_theta(55 * DEG).increment_phi(-8 * DEG),
            run_time=ORBIT_SECONDS,
        )

        cross_section = ParametricCurve(
            lambda x: axes.c2p(x, 0, SURFACE_AMPLITUDE * np.sin(x)),
            t_range=(-PI, PI, 0.05),
        ).set_stroke(YELLOW, width=6)
        slice_equation = Tex(
            R"f(x,0)=\frac{3}{4}\sin(x)",
            font_size=42,
        ).move_to(equation)
        slice_equation.fix_in_frame()
        marker = Sphere(radius=0.1, color=YELLOW).move_to(cross_section.get_start())

        self.play(
            ShowCreation(cross_section),
            Transform(equation, slice_equation),
            GrowFromCenter(marker),
            run_time=SLICE_SECONDS,
        )
        self.play(
            MoveAlongPath(marker, cross_section),
            self.frame.animate.increment_theta(35 * DEG),
            run_time=TRACE_SECONDS,
            rate_func=smooth,
        )
        self.wait(HOLD_SECONDS)
        self.play(
            FadeOut(axes),
            FadeOut(surface),
            FadeOut(mesh),
            FadeOut(cross_section),
            FadeOut(marker),
            FadeOut(title),
            FadeOut(equation),
            run_time=OUTRO_SECONDS,
        )
