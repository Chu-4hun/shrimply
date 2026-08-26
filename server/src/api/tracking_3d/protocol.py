from typing import Annotated, Literal

import msgspec
from pydantic import BaseModel, ConfigDict, Field, TypeAdapter, model_validator

type ModelId = Literal["colmap/colmap", "MIT-SPARK/VGGT-SLAM"]
type WorkerState = Literal["stopped", "loading", "analyzing", "ready"]
type ColmapQuality = Literal["low", "medium", "high", "extreme"]
type ColmapCameraModel = Literal[
    "simple_radial", "pinhole", "open_cv", "open_cv_fisheye", "equirectangular"
]
type Projection = Literal["perspective", "fisheye", "equirectangular"]

MODEL_IDS = ("colmap/colmap", "MIT-SPARK/VGGT-SLAM")
ARCHIVE_MAGIC = b"SHRM3D01"
MAXIMUM_HEADER_BYTES = 1024 * 1024
MAXIMUM_JPEG_BYTES = 64 * 1024 * 1024
MAXIMUM_ARCHIVE_BYTES = 8 * 1024**3


class ProtocolModel(BaseModel):
    model_config = ConfigDict(extra="forbid", strict=True)


class AnalysisRequest(ProtocolModel):
    version: Literal[1]
    model: ModelId
    frame_count: int = Field(gt=1)
    quality: ColmapQuality | None = None
    camera_model: ColmapCameraModel | None = None

    @model_validator(mode="after")
    def validate_backend_settings(self) -> "AnalysisRequest":
        colmap = self.model == "colmap/colmap"
        if colmap and (self.quality is None or self.camera_model is None):
            raise ValueError("COLMAP quality and camera model are required")
        if not colmap and (self.quality is not None or self.camera_model is not None):
            raise ValueError("COLMAP settings cannot be used with this backend")
        return self


ANALYSIS_REQUEST_VALIDATOR = TypeAdapter(AnalysisRequest)


class ProgressEvent(ProtocolModel):
    kind: Literal["progress"] = "progress"
    message: str
    completed_frames: int = Field(ge=0)
    total_frames: int = Field(gt=0)


class QueuedEvent(ProtocolModel):
    kind: Literal["queued"] = "queued"
    position: int = Field(gt=0)


class CameraEvent(ProtocolModel):
    kind: Literal["camera"] = "camera"
    frame_index: int = Field(ge=0)
    camera_from_world_rotation: list[float] = Field(min_length=4, max_length=4)
    camera_from_world_translation: list[float] = Field(min_length=3, max_length=3)
    projection: Projection
    image_width: int = Field(gt=0)
    image_height: int = Field(gt=0)
    focal_y: float | None

    @model_validator(mode="after")
    def validate_intrinsics(self) -> "CameraEvent":
        if self.projection == "equirectangular":
            if self.focal_y is not None:
                raise ValueError("Equirectangular cameras cannot have a focal length")
        elif self.focal_y is None or self.focal_y <= 0:
            raise ValueError("Perspective and fisheye cameras require a focal length")
        return self


class ResultEvent(ProtocolModel):
    kind: Literal["result"] = "result"
    camera_count: int = Field(gt=1)


class ErrorEvent(ProtocolModel):
    kind: Literal["error"] = "error"
    code: str
    message: str


type WorkerEvent = Annotated[
    ProgressEvent | CameraEvent | ResultEvent | ErrorEvent,
    Field(discriminator="kind"),
]
type PublicEvent = QueuedEvent | WorkerEvent
WORKER_EVENT_VALIDATOR = TypeAdapter(WorkerEvent)


def encode_event(event: WorkerEvent) -> bytes:
    return msgspec.msgpack.encode(event.model_dump(mode="python"))


def decode_event(payload: bytes) -> WorkerEvent:
    return WORKER_EVENT_VALIDATOR.validate_python(msgspec.msgpack.decode(payload))
