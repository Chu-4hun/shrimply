from dataclasses import dataclass
from pathlib import Path

DEFAULT_WEIGHT_ROOT = Path("models")


@dataclass(frozen=True, slots=True)
class SafetensorsJsonPair:
    safetensors: Path
    json: Path

    @property
    def stem_path(self) -> Path:
        return self.safetensors.with_suffix("")

    @property
    def legacy_pth(self) -> Path:
        return self.stem_path.with_suffix(".pth")

    def exists(self) -> bool:
        return self.safetensors.exists()


def safetensors_json_pair(path: Path) -> SafetensorsJsonPair:
    stem_path = path.with_suffix("") if path.suffix else path
    return SafetensorsJsonPair(
        safetensors=stem_path.with_suffix(".safetensors"),
        json=stem_path.with_suffix(".json"),
    )
