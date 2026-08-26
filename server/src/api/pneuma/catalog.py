import logging
from pathlib import Path

from safetensors import SafetensorError, safe_open

import env

from api.pneuma.protocol import ModelInfo, ModelMetadata

logger = logging.getLogger("shrimply.pneuma")
MODEL_DIRECTORY = env.PNEUMA_MODEL_DIRECTORY
WEIGHT_EXTENSIONS = frozenset((".safetensors", ".pth"))


def model_metadata(path: Path) -> ModelMetadata:
    if path.suffix != ".safetensors":
        return ModelMetadata()
    try:
        with safe_open(path, framework="pt", device="cpu") as checkpoint:
            metadata = checkpoint.metadata() or {}
    except (OSError, SafetensorError) as exception:
        logger.warning("Could not read Pneuma model metadata path=%s: %s", path, exception)
        return ModelMetadata()
    return ModelMetadata(
        experiment_name=metadata.get("experiment_name"),
        version=metadata.get("version"),
        saved_at=metadata.get("saved_at"),
    )


def models() -> list[ModelInfo]:
    found: dict[str, ModelInfo] = {}
    try:
        entries = tuple(MODEL_DIRECTORY.iterdir()) if MODEL_DIRECTORY.is_dir() else ()
    except OSError as exception:
        logger.warning(
            "Could not read Pneuma model directory path=%s: %s",
            MODEL_DIRECTORY,
            exception,
        )
        entries = ()
    for path in sorted(
        (
            path
            for path in entries
            if path.is_file()
            and path.suffix in WEIGHT_EXTENSIONS
            and path.stem != "none"
        ),
        key=lambda path: (path.stem, path.suffix != ".safetensors", path.name),
    ):
        found.setdefault(
            path.stem,
            ModelInfo(name=path.stem, metadata=model_metadata(path)),
        )
    return [*found.values(), ModelInfo(name="none")]
