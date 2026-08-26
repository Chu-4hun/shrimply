from typing import TYPE_CHECKING

from api.pneuma.inference.utils.types import HUBERT_LARGE_VERSIONS, ModelVersion
from api.pneuma.inference.utils.types.version import hubert_output_layer

if TYPE_CHECKING:
    from api.pneuma.inference.utils.hubert import HubertModelWrapper


def content_model_for_version(version: ModelVersion) -> "HubertModelWrapper":
    if version in HUBERT_LARGE_VERSIONS:
        from api.pneuma.inference.utils.hubert import get_hubert_large

        return get_hubert_large(output_layer=hubert_output_layer(version))
    from api.pneuma.inference.utils.hubert import get_hubert

    return get_hubert()
