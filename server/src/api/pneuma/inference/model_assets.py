import logging
import tempfile
from collections.abc import Callable
from pathlib import Path

logger = logging.getLogger("shrimply.pneuma")

# LegacyAssetDownloader takes the temporary directory Path to save the downloaded file,
# and returns the Path of the downloaded file.
type LegacyAssetDownloader = Callable[[Path], Path]
type LegacyAssetConverter = Callable[[Path, Path], Path | None]


def resolve_safetensors_asset(
    *,
    safetensors_path: Path,
    convert_legacy: LegacyAssetConverter,
    asset_name: str,
    download_legacy: LegacyAssetDownloader | None = None,
) -> Path:
    if safetensors_path.exists():
        return safetensors_path

    if download_legacy is None:
        raise FileNotFoundError(
            f"{asset_name} is missing safetensors weights and no downloader is provided: "
            f"{safetensors_path}"
        )

    with tempfile.TemporaryDirectory() as temp_dir_str:
        temp_dir = Path(temp_dir_str)
        logger.info(
            "Downloading %s legacy weights to temporary directory...", asset_name
        )
        source_path = download_legacy(temp_dir)
        if not source_path.exists():
            raise RuntimeError(
                f"Downloader for {asset_name} returned non-existent path: {source_path}"
            )

        logger.info(
            "Converting %s legacy weights from %s to %s",
            asset_name,
            source_path,
            safetensors_path,
        )
        safetensors_path.parent.mkdir(parents=True, exist_ok=True)
        converted_path = convert_legacy(source_path, safetensors_path)
        output_path = converted_path or safetensors_path
        if not output_path.exists():
            raise RuntimeError(
                f"Converting {asset_name} did not create safetensors file: {output_path}"
            )

    return output_path
