from __future__ import annotations

from cyclopts import App
from cyclopts.types import ResolvedExistingFile

from shrimply_manim.scene_discovery import discover


app = App(
    name="shrimply-manim",
    help="Inspect Manim sources compiled by Shrimply.",
)


@app.command
def scenes(source: ResolvedExistingFile) -> None:
    """List Scene classes without importing or rendering the source."""
    for name in discover(source):
        print(name)
