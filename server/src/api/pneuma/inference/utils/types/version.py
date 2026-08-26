from typing import Literal, TypeAlias

ModelVersion: TypeAlias = Literal["v2", "v3", "v3.2", "v3.3"]
SUPPORTED_VERSIONS = frozenset(("v2", "v3", "v3.2", "v3.3"))
HUBERT_LARGE_VERSIONS = frozenset(("v3", "v3.2", "v3.3"))


def hubert_output_layer(version: ModelVersion) -> int:
    return 9 if version in {"v3.2", "v3.3"} else 12
