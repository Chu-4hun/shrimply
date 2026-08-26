"""Editor-reflected parameters for Shrimply Manim scenes."""

from __future__ import annotations

import math
from collections.abc import Callable, Sequence
from fractions import Fraction
from typing import TypeGuard

from shrimply_manim import adw
from shrimply_manim.worker_types import Message, MessageValue, ParameterOverrides


_overrides: ParameterOverrides | None = None
_parameters: list[Message] = []
_keys: set[str] = set()
_render_is_current = True


def begin(overrides: ParameterOverrides | None = None) -> None:
    global _overrides, _parameters, _keys, _render_is_current
    _overrides = {} if overrides is None else overrides
    _parameters = []
    _keys = set()
    _render_is_current = True


def finish() -> tuple[list[Message], bool]:
    global _overrides
    reflected = _parameters
    _overrides = None
    return reflected, _render_is_current


def cancel() -> None:
    global _overrides
    _overrides = None


def _identity(kind: str, key: str | None, label: str | None) -> tuple[str, str]:
    ordinal = len(_parameters) + 1
    resolved_key = key or f"parameter-{ordinal}"
    resolved_label = label or key or f"{kind.title()} {ordinal}"
    if not resolved_key:
        raise ValueError("Manim parameter key must not be empty")
    if not resolved_label:
        raise ValueError("Manim parameter label must not be empty")
    if resolved_key in _keys:
        raise ValueError(f"duplicate Manim parameter key {resolved_key!r}")
    _keys.add(resolved_key)
    return resolved_key, resolved_label


def _reflect[Value: MessageValue](
    kind: str,
    default: Value,
    control: Message,
    key: str | None,
    label: str | None,
    valid_override: Callable[[MessageValue], TypeGuard[Value]],
) -> Value:
    global _render_is_current
    if _overrides is None:
        return default
    resolved_key, resolved_label = _identity(kind, key, label)
    override = _overrides.get(resolved_key)
    value = default
    if isinstance(override, dict) and override.get("kind") == kind:
        candidate = override.get("value")
        if valid_override(candidate):
            value = candidate
        else:
            _render_is_current = False
    else:
        _render_is_current = False
    _parameters.append(
        {
            "key": resolved_key,
            "label": resolved_label,
            "default": {"kind": kind, "value": default},
            "value": {"kind": kind, "value": value},
            "control": control,
        }
    )
    return value


def use_int(
    default: int = 0,
    *,
    min: int | None = None,
    max: int | None = None,
    step: int = 1,
    key: str | None = None,
    label: str | None = None,
) -> int:
    """Return an integer exposed as a numeric editor control."""
    if isinstance(default, bool) or not isinstance(default, int):
        raise TypeError("use_int default must be an int")
    if min is not None and (isinstance(min, bool) or not isinstance(min, int)):
        raise TypeError("use_int min must be an int or None")
    if max is not None and (isinstance(max, bool) or not isinstance(max, int)):
        raise TypeError("use_int max must be an int or None")
    if isinstance(step, bool) or not isinstance(step, int) or step <= 0:
        raise ValueError("use_int step must be a positive int")
    if min is not None and max is not None and min > max:
        raise ValueError("use_int min must not exceed max")
    if min is not None and default < min or max is not None and default > max:
        raise ValueError("use_int default is outside its range")

    def valid(value: MessageValue) -> TypeGuard[int]:
        return (
            isinstance(value, int)
            and not isinstance(value, bool)
            and (min is None or value >= min)
            and (max is None or value <= max)
        )

    return _reflect(
        "integer",
        default,
        {"kind": "integer", "minimum": min, "maximum": max, "step": step},
        key,
        label,
        valid,
    )


def use_float(
    default: float = 0.0,
    *,
    min: float | None = None,
    max: float | None = None,
    step: float = 0.1,
    key: str | None = None,
    label: str | None = None,
) -> float:
    """Return a float exposed as a numeric editor control."""
    default = _finite_float(default, "default")
    minimum = None if min is None else _finite_float(min, "min")
    maximum = None if max is None else _finite_float(max, "max")
    step = _finite_float(step, "step")
    if step <= 0:
        raise ValueError("use_float step must be positive")
    if minimum is not None and maximum is not None and minimum > maximum:
        raise ValueError("use_float min must not exceed max")
    if minimum is not None and default < minimum or maximum is not None and default > maximum:
        raise ValueError("use_float default is outside its range")

    def valid(value: MessageValue) -> TypeGuard[float]:
        return (
            isinstance(value, float)
            and math.isfinite(value)
            and (minimum is None or value >= minimum)
            and (maximum is None or value <= maximum)
        )

    return float(
        _reflect(
            "float",
            default,
            {
                "kind": "float",
                "minimum": minimum,
                "maximum": maximum,
                "step": step,
            },
            key,
            label,
            valid,
        )
    )


def use_fraction(
    default: Fraction = Fraction(0),
    *,
    key: str | None = None,
    label: str | None = None,
) -> Fraction:
    """Return an exact fraction exposed as a decimal editor control."""
    global _render_is_current
    if not isinstance(default, Fraction):
        raise TypeError("use_fraction default must be a fractions.Fraction")

    def encoded(value: Fraction) -> dict[str, int]:
        return {"numerator": value.numerator, "denominator": value.denominator}

    def valid(value: MessageValue) -> TypeGuard[dict[str, int]]:
        return (
            isinstance(value, dict)
            and set(value) == {"numerator", "denominator"}
            and isinstance(value["numerator"], int)
            and not isinstance(value["numerator"], bool)
            and isinstance(value["denominator"], int)
            and not isinstance(value["denominator"], bool)
            and value["denominator"] != 0
        )

    if _overrides is None:
        return default

    resolved_key, resolved_label = _identity("fraction", key, label)
    override = _overrides.get(resolved_key)
    value = default
    if isinstance(override, dict):
        kind = override.get("kind")
        raw_value = override.get("value")
        if kind == "fraction" and valid(raw_value):
            value = Fraction(raw_value["numerator"], raw_value["denominator"])
            if raw_value != encoded(value):
                _render_is_current = False
        elif (
            kind == "float"
            and isinstance(raw_value, (int, float))
            and not isinstance(raw_value, bool)
            and math.isfinite(raw_value)
        ):
            value = Fraction(str(raw_value))
            _render_is_current = False
        elif (
            kind == "integer"
            and isinstance(raw_value, int)
            and not isinstance(raw_value, bool)
        ):
            value = Fraction(raw_value)
            _render_is_current = False
        else:
            _render_is_current = False
    else:
        _render_is_current = False

    _parameters.append(
        {
            "key": resolved_key,
            "label": resolved_label,
            "default": {"kind": "fraction", "value": encoded(default)},
            "value": {"kind": "fraction", "value": encoded(value)},
            "control": {"kind": "fraction"},
        }
    )
    return value


def use_color(
    default: str = "blue3",
    *,
    key: str | None = None,
    label: str | None = None,
) -> str:
    """Return an RGB hex color exposed as an editor color picker."""
    default_color = _color(default)

    def valid(value: MessageValue) -> TypeGuard[dict[str, int]]:
        return (
            isinstance(value, dict)
            and set(value) == {"r", "g", "b", "a"}
            and all(isinstance(channel, int) and 0 <= channel <= 255 for channel in value.values())
        )

    rgba = _reflect(
        "color",
        _rgba(default_color),
        {"kind": "color"},
        key,
        label,
        valid,
    )
    return f"#{rgba['r']:02x}{rgba['g']:02x}{rgba['b']:02x}"


def use_option(
    options: Sequence[str],
    default: str | None = None,
    *,
    key: str | None = None,
    label: str | None = None,
) -> str:
    """Return one string from a reflected list of options."""
    values = list(options)
    if not values or any(not isinstance(value, str) for value in values):
        raise ValueError("use_option options must be a nonempty sequence of strings")
    if len(set(values)) != len(values):
        raise ValueError("use_option options must be unique")
    default = values[0] if default is None else default
    if default not in values:
        raise ValueError("use_option default must be one of its options")
    return _reflect(
        "option",
        default,
        {"kind": "option", "options": values},
        key,
        label,
        _string_in(values),
    )


def reflect_anti_aliasing(default: int) -> int:
    """Reflect Manim's camera sample count as the editor anti-aliasing control."""
    global _render_is_current
    if isinstance(default, bool) or not isinstance(default, int):
        raise TypeError("Manim camera samples must be an int")
    default = 0 if default in (0, 1) else default
    if default not in (0, 2, 4, 8, 16):
        raise ValueError("Manim camera samples must be 0, 1, 2, 4, 8, or 16")
    if _overrides is None:
        return default

    key, label = _identity("integer", "shrimply-anti-aliasing", "Anti-aliasing")
    override = _overrides.get(key)
    value = default
    if override is not None:
        if (
            isinstance(override, dict)
            and override.get("kind") == "integer"
            and override.get("value") in (0, 2, 4, 8, 16)
        ):
            value = override["value"]
        else:
            _render_is_current = False
    _parameters.append(
        {
            "key": key,
            "label": label,
            "default": {"kind": "integer", "value": default},
            "value": {"kind": "integer", "value": value},
            "control": {"kind": "anti_aliasing"},
        }
    )
    return value


def use_bool(
    default: bool = False,
    *,
    key: str | None = None,
    label: str | None = None,
) -> bool:
    """Return a boolean exposed as an editor switch."""
    if not isinstance(default, bool):
        raise TypeError("use_bool default must be a bool")
    return _reflect(
        "boolean",
        default,
        {"kind": "boolean"},
        key,
        label,
        _is_bool,
    )


def use_string(
    default: str = "",
    *,
    key: str | None = None,
    label: str | None = None,
) -> str:
    """Return a string exposed as an editor text field."""
    if not isinstance(default, str):
        raise TypeError("use_string default must be a string")
    return _reflect(
        "string",
        default,
        {"kind": "string"},
        key,
        label,
        _is_string,
    )


def _finite_float(value: float, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError(f"use_float {name} must be a number")
    value = float(value)
    if not math.isfinite(value):
        raise ValueError(f"use_float {name} must be finite")
    return value


def _color(value: str) -> str:
    if not isinstance(value, str):
        raise TypeError("use_color default must be a string")
    normalized = value.replace("_", "").replace("-", "").upper()
    color = adw.PALETTE.get(normalized, value)
    if len(color) != 7 or not color.startswith("#"):
        raise ValueError(f"unknown color {value!r}")
    try:
        int(color[1:], 16)
    except ValueError as error:
        raise ValueError(f"invalid color {value!r}") from error
    return color.lower()


def _rgba(color: str) -> dict[str, int]:
    return {
        "r": int(color[1:3], 16),
        "g": int(color[3:5], 16),
        "b": int(color[5:7], 16),
        "a": 255,
    }


def _string_in(values: list[str]) -> Callable[[MessageValue], TypeGuard[str]]:
    def valid(value: MessageValue) -> TypeGuard[str]:
        return isinstance(value, str) and value in values

    return valid


def _is_bool(value: MessageValue) -> TypeGuard[bool]:
    return isinstance(value, bool)


def _is_string(value: MessageValue) -> TypeGuard[str]:
    return isinstance(value, str)
