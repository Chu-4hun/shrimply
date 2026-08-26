"""Adwaita colors for Manim scenes."""

from typing import Final


BLUE_1: Final = "#99c1f1"
BLUE_2: Final = "#62a0ea"
BLUE_3: Final = "#3584e4"
BLUE_4: Final = "#1c71d8"
BLUE_5: Final = "#1a5fb4"

GREEN_1: Final = "#8ff0a4"
GREEN_2: Final = "#57e389"
GREEN_3: Final = "#33d17a"
GREEN_4: Final = "#2ec27e"
GREEN_5: Final = "#26a269"

YELLOW_1: Final = "#f9f06b"
YELLOW_2: Final = "#f8e45c"
YELLOW_3: Final = "#f6d32d"
YELLOW_4: Final = "#f5c211"
YELLOW_5: Final = "#e5a50a"

ORANGE_1: Final = "#ffbe6f"
ORANGE_2: Final = "#ffa348"
ORANGE_3: Final = "#ff7800"
ORANGE_4: Final = "#e66100"
ORANGE_5: Final = "#c64600"

RED_1: Final = "#f66151"
RED_2: Final = "#ed333b"
RED_3: Final = "#e01b24"
RED_4: Final = "#c01c28"
RED_5: Final = "#a51d2d"

PURPLE_1: Final = "#dc8add"
PURPLE_2: Final = "#c061cb"
PURPLE_3: Final = "#9141ac"
PURPLE_4: Final = "#813d9c"
PURPLE_5: Final = "#613583"

BROWN_1: Final = "#cdab8f"
BROWN_2: Final = "#b5835a"
BROWN_3: Final = "#986a44"
BROWN_4: Final = "#865e3c"
BROWN_5: Final = "#63452c"

LIGHT_1: Final = "#ffffff"
LIGHT_2: Final = "#f6f5f4"
LIGHT_3: Final = "#deddda"
LIGHT_4: Final = "#c0bfbc"
LIGHT_5: Final = "#9a9996"

DARK_1: Final = "#77767b"
DARK_2: Final = "#5e5c64"
DARK_3: Final = "#3d3846"
DARK_4: Final = "#241f31"
DARK_5: Final = "#000000"

VIEW_BG_LIGHT: Final = "#ffffff"
VIEW_BG_DARK: Final = "#1d1d20"
VIEW_FG_LIGHT: Final = "#333337"
VIEW_FG_DARK: Final = "#ffffff"

PALETTE: Final[dict[str, str]] = {
    f"{family}{index}": color
    for family, colors in (
        ("BLUE", (BLUE_1, BLUE_2, BLUE_3, BLUE_4, BLUE_5)),
        ("GREEN", (GREEN_1, GREEN_2, GREEN_3, GREEN_4, GREEN_5)),
        ("YELLOW", (YELLOW_1, YELLOW_2, YELLOW_3, YELLOW_4, YELLOW_5)),
        ("ORANGE", (ORANGE_1, ORANGE_2, ORANGE_3, ORANGE_4, ORANGE_5)),
        ("RED", (RED_1, RED_2, RED_3, RED_4, RED_5)),
        ("PURPLE", (PURPLE_1, PURPLE_2, PURPLE_3, PURPLE_4, PURPLE_5)),
        ("BROWN", (BROWN_1, BROWN_2, BROWN_3, BROWN_4, BROWN_5)),
        ("LIGHT", (LIGHT_1, LIGHT_2, LIGHT_3, LIGHT_4, LIGHT_5)),
        ("DARK", (DARK_1, DARK_2, DARK_3, DARK_4, DARK_5)),
    )
    for index, color in enumerate(colors, start=1)
}
