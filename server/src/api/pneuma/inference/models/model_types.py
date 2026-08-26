from typing import TYPE_CHECKING

from torch import nn

if TYPE_CHECKING:
    from api.pneuma.inference.models.models_v2 import SynthesizerTrnNSFsid
    from api.pneuma.inference.models.models_v3 import SynthesizerTrnBigVGANsid
    from api.pneuma.inference.models.models_v32 import SynthesizerTrnBigVGANV32
    from api.pneuma.inference.models.models_v33 import SynthesizerTrnBigVGANV33

    type PneumaModel = (
        SynthesizerTrnNSFsid
        | SynthesizerTrnBigVGANsid
        | SynthesizerTrnBigVGANV32
        | SynthesizerTrnBigVGANV33
    )
else:
    type PneumaModel = nn.Module
