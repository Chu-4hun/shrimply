from pathlib import Path


PROJECT_CACHE_DIR = Path.cwd() / ".cache"
MODEL_CACHE_DIR = PROJECT_CACHE_DIR / "models"


def model_cache_dir(name: str) -> Path:
    return MODEL_CACHE_DIR / name
