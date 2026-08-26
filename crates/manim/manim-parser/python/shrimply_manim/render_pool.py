from __future__ import annotations

import re
from dataclasses import dataclass
from functools import cache
from typing import cast

import numpy as np
from manimlib.mobject.mobject import Mobject
from manimlib.mobject.types.surface import Surface
from manimlib.camera.camera import Camera
from manimlib.renderer.drawing import (
    DEFAULT,
    FILL_BORDER,
    WINDING_COUNT,
    WINDING_COVER,
    Drawing,
    SurfaceDrawing,
    VDrawing,
)
from manimlib.renderer.pipeline import PipelineState
from manimlib.renderer.gpu import FRAME_DTYPE
from manimlib.renderer.shader_source import FIRST_TEXTURE_BINDING, get_shader_code
from manimlib.renderer.uniform_block import Uniforms
from manimlib.scene.scene import Scene

from shrimply_manim.worker_types import Message, MessageValue


type ModuleSpec = tuple[str, str, dict[str, str]]
type StencilOps = tuple[str, str, str]


@dataclass(frozen=True, slots=True)
class PreparedPipelineState:
    depth_test: bool
    depth_write: bool
    color_write: bool
    stencil_compare: str
    stencil_front: StencilOps
    stencil_back: StencilOps


@dataclass(frozen=True, slots=True)
class PreparedTexture:
    binding: int
    name: str
    path: str


@dataclass(frozen=True, slots=True)
class PreparedDraw:
    source: str
    state: PreparedPipelineState
    textures: tuple[PreparedTexture, ...]
    vertices: int
    indices: tuple[int, ...]
    data: bytes
    uniforms: bytes


@dataclass(frozen=True, slots=True)
class PreparedFrame:
    draws: tuple[PreparedDraw, ...]
    camera_uniforms: Message
    camera_bytes: bytes
    background_rgba: tuple[float, float, float, float]


@dataclass(slots=True)
class CaptureGpu:
    frame_uniforms: Uniforms


def install_capture() -> None:
    def init_renderer(camera: Camera) -> None:
        camera.gpu = CaptureGpu(Uniforms(FRAME_DTYPE))
        camera.renderer = None

    def init_target(camera: Camera) -> None:
        camera.pixel_shape = camera.get_target_shape()

    def capture(camera: Camera, *_mobjects: Mobject) -> None:
        camera.resize_target()
        camera.refresh_uniforms()

    Camera.init_renderer = init_renderer
    Camera.init_target = init_target
    Camera.capture = capture


def _plain(value: np.generic | np.ndarray) -> MessageValue:
    if isinstance(value, np.ndarray):
        return cast(MessageValue, value.tolist())
    return cast(MessageValue, value.item())


def _state(state: PipelineState, depth_test: bool) -> PreparedPipelineState:
    resolved = state.resolved(depth_test)
    front, back = resolved.stencil_ops
    return PreparedPipelineState(
        depth_test=bool(resolved.depth_test),
        depth_write=resolved.depth_write,
        color_write=resolved.color_write,
        stencil_compare=resolved.stencil_compare,
        stencil_front=front,
        stencil_back=back,
    )


@cache
def _module(
    filename: str,
    data_dtype: np.dtype[np.void],
    uniform_dtype: np.dtype[np.void],
    texture_names: tuple[str, ...],
    replacements: tuple[tuple[str, str], ...],
) -> str:
    source = get_shader_code(filename, data_dtype, uniform_dtype, texture_names)
    if source is None:
        raise RuntimeError(f"Manim shader {filename!r} was not found")
    for old, new in replacements:
        source = re.sub(old, new, source)
    return source


def _modules(mobject: Mobject, drawing_class: type[Drawing]) -> dict[str, str]:
    specs = cast(list[ModuleSpec], drawing_class.module_specs(mobject))
    texture_names = tuple(mobject.texture_paths)
    return {
        name: _module(
            filename,
            mobject.data.dtype,
            mobject.uniforms.dtype,
            texture_names,
            tuple(replacements.items()),
        )
        for name, filename, replacements in specs
    }


def _textures(mobject: Mobject) -> tuple[PreparedTexture, ...]:
    return tuple(
        PreparedTexture(FIRST_TEXTURE_BINDING + index, name, path)
        for index, (name, path) in enumerate(mobject.texture_paths.items())
    )


def _draw(
    mobject: Mobject,
    source: str,
    state: PipelineState,
    vertices: int,
    indices: tuple[int, ...] = (),
    data: bytes | None = None,
) -> PreparedDraw:
    return PreparedDraw(
        source=source,
        state=_state(state, bool(mobject.depth_test)),
        textures=_textures(mobject),
        vertices=vertices,
        indices=indices,
        data=mobject.data.bytes.tobytes() if data is None else data,
        uniforms=mobject.uniforms.bytes.tobytes(),
    )


def _has_fill(mobject: Mobject) -> bool:
    return bool(
        mobject.uniforms["fill_rgba"][3] or mobject.uniforms["fill_rgba_end"][3]
    )


def _vector_run_data(run: list[Mobject]) -> tuple[int, bytes]:
    if len(run) == 1:
        return len(run[0].data), run[0].data.bytes.tobytes()
    record_size = run[0].data.dtype.itemsize
    result = bytearray()
    for index, mobject in enumerate(run):
        data = mobject.data.bytes.tobytes()
        result.extend(data)
        if index + 1 != len(run):
            result.extend(data[-record_size:])
    return sum(len(mobject.data) for mobject in run) + len(run) - 1, bytes(result)


def _vector_draws(run: list[Mobject], modules: dict[str, str]) -> list[PreparedDraw]:
    mobject = run[0]
    records, data = _vector_run_data(run)
    curves = records // 2
    stroke_vertices = VDrawing.stroke_verts_per_curve * curves
    fill: list[PreparedDraw] = []
    if _has_fill(mobject):
        fill_vertices = VDrawing.fill_verts_per_curve * curves
        fill = [
            _draw(mobject, modules["fill"], WINDING_COUNT, fill_vertices, data=data),
            _draw(
                mobject,
                modules["border"],
                FILL_BORDER,
                VDrawing.stroke_verts_per_curve * (curves + 1),
                data=data,
            ),
            _draw(mobject, modules["fill"], WINDING_COVER, fill_vertices, data=data),
        ]
    stroke = [_draw(mobject, modules["stroke"], DEFAULT, stroke_vertices, data=data)]
    return stroke + fill if mobject.stroke_behind else fill + stroke


def _surface_indices(mobject: Surface, camera_position: np.ndarray) -> tuple[int, ...]:
    if not (mobject.sort_to_camera or not mobject.is_opaque()):
        return ()
    first_vertices, middles = mobject.get_triangles()
    if len(first_vertices) == 0:
        return ()
    offsets = middles - camera_position
    order = np.argsort(-np.einsum("ij,ij->i", offsets, offsets))
    vertices = first_vertices[order, np.newaxis] + np.arange(3)
    return tuple(int(index) for index in vertices.reshape(-1))


def _mobjects(scene: Scene) -> list[Mobject]:
    drawn: list[Mobject] = []
    ascending = True
    previous_z = 0
    for mobject in scene.mobjects:
        for member in mobject.get_family():
            drawing_class = cast(type[Drawing], member.drawing_class)
            if len(member.data) == 0 or not drawing_class.draws(member):
                continue
            drawn.append(member)
            if member.z_index < previous_z:
                ascending = False
            previous_z = member.z_index
    if not ascending:
        drawn.sort(key=lambda mobject: mobject.z_index)
    return drawn


def _can_join_vector_run(previous: Mobject, current: Mobject) -> bool:
    drawing_class = cast(type[Drawing], current.drawing_class)
    previous_class = cast(type[Drawing], previous.drawing_class)
    if not (
        issubclass(drawing_class, VDrawing)
        and drawing_class.key(current) == previous_class.key(previous)
        and current.uniforms.bytes.tobytes() == previous.uniforms.bytes.tobytes()
        and current.depth_test == previous.depth_test
        and current.stroke_behind == previous.stroke_behind
    ):
        return False
    if not (_has_fill(current) or _has_fill(previous)):
        return True
    return current.fill_group is not None and current.fill_group is previous.fill_group


def _runs(mobjects: list[Mobject]) -> list[list[Mobject]]:
    runs: list[list[Mobject]] = []
    for mobject in mobjects:
        if runs and _can_join_vector_run(runs[-1][-1], mobject):
            runs[-1].append(mobject)
        else:
            runs.append([mobject])
    return runs


def prepare_frame(scene: Scene) -> PreparedFrame:
    scene.camera.refresh_uniforms()
    frame_uniforms = scene.camera.gpu.frame_uniforms
    camera_position = np.asarray(frame_uniforms["camera_position"], dtype=np.float32)
    draws: list[PreparedDraw] = []
    for run in _runs(_mobjects(scene)):
        mobject = run[0]
        drawing_class = cast(type[Drawing], mobject.drawing_class)
        modules = _modules(mobject, drawing_class)
        if issubclass(drawing_class, VDrawing):
            draws.extend(_vector_draws(run, modules))
            continue
        indices = (
            _surface_indices(cast(Surface, mobject), camera_position)
            if issubclass(drawing_class, SurfaceDrawing)
            else ()
        )
        vertices = len(indices) or mobject.verts_per_record * len(mobject.data)
        draws.append(_draw(mobject, modules["main"], DEFAULT, vertices, indices))

    camera = {
        name: _plain(cast(np.generic | np.ndarray, frame_uniforms[name]))
        for name in frame_uniforms.dtype.names or ()
        if not name.startswith("_")
    }
    return PreparedFrame(
        tuple(draws),
        camera,
        frame_uniforms.bytes.tobytes(),
        tuple(float(component) for component in scene.camera.background_rgba),
    )
