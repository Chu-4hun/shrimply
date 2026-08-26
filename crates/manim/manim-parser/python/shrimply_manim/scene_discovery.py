import ast
import sys
from pathlib import Path

import msgpack


def base_name(node: ast.expr) -> str | None:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        return node.attr
    return None


def discover(source: Path) -> list[str]:
    module = ast.parse(source.read_text(), filename=str(source))
    classes = [node for node in module.body if isinstance(node, ast.ClassDef)]
    scene_names: set[str] = set()
    while True:
        discovered = {
            node.name
            for node in classes
            if any(
                name is not None and (name.endswith("Scene") or name in scene_names)
                for name in map(base_name, node.bases)
            )
        }
        if discovered <= scene_names:
            return [node.name for node in classes if node.name in scene_names]
        scene_names.update(discovered)


if __name__ == "__main__":
    sys.stdout.buffer.write(msgpack.packb(discover(Path(sys.argv[1])), use_bin_type=True))
