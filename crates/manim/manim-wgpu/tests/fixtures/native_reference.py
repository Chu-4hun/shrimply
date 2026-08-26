from __future__ import annotations

import importlib.util
import sys
from fractions import Fraction
from pathlib import Path
from types import ModuleType

from manimlib import manim_config
from manimlib.scene.scene import Scene
from manimlib.scene.scene_file_writer import SceneFileWriter


class RawFrameWriter(SceneFileWriter):
    def __init__(self, scene: Scene, output: Path, width: int, height: int) -> None:
        super().__init__(scene, quiet=True)
        self.output = output.open("wb")

    def begin(self) -> None:
        pass

    def finish(self) -> None:
        self.output.close()

    def write_frame(self) -> None:
        self.output.write(self.scene.camera.get_frame_bytes())


def load_module(source: Path) -> ModuleType:
    sys.path.insert(0, str(source.parent))
    spec = importlib.util.spec_from_file_location("shrimply_native_reference_scene", source)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"could not load {source}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def scene_class(module: ModuleType, name: str) -> type[Scene]:
    value = vars(module).get(name)
    if not isinstance(value, type) or not issubclass(value, Scene):
        raise RuntimeError(f"{name!r} is not a Manim Scene")
    return value


def main() -> None:
    if len(sys.argv) != 7:
        raise RuntimeError(
            "usage: native_reference.py SOURCE SCENE OUTPUT WIDTH HEIGHT FPS"
        )
    source = Path(sys.argv[1])
    name = sys.argv[2]
    output = Path(sys.argv[3])
    width = int(sys.argv[4])
    height = int(sys.argv[5])
    fps = Fraction(sys.argv[6])
    manim_config.camera.resolution = (width, height)
    scene = scene_class(load_module(source), name)(
        camera_config={
            "resolution": (width, height),
            "fps": float(fps),
            "background_opacity": 0.0,
        },
        file_writer_config={
            "write_to_movie": False,
            "save_last_frame": False,
            "quiet": True,
        },
        skip_animations=False,
    )
    scene.file_writer = RawFrameWriter(scene, output, width, height)
    scene.run()


if __name__ == "__main__":
    main()
