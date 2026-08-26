import logging
import subprocess
import tomllib
from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager
from http import HTTPStatus
from pathlib import Path

import env
from fastapi import Request
from fastapi.responses import Response
from gradio import Server

from api import (
    gpu,
    log_request,
    resource,
    respond,
    respond_error,
)
from api.pneuma.catalog import models as pneuma_models
from api.pneuma.http import handle_request as handle_pneuma_request
from api.pneuma.protocol import ModelsResponse as PneumaModelsResponse
from api.sam2.http import handle_request as handle_sam2_request
from api.status import server_status
from api.stt.http import handle_request as handle_stt_request
from api.tracking_3d.http import handle_request as handle_tracking_3d_request
from api.tts.catalog import MODELS
from api.tts.http import handle_request as handle_tts_request
from api.tts.protocol import ModelsResponse
from api.video_generation.catalog import MODELS as VIDEO_GENERATION_MODELS
from api.video_generation.http import handle_request as handle_video_generation_request
from api.video_generation.protocol import ModelsResponse as VideoGenerationModelsResponse

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s %(levelname)s %(name)s: %(message)s",
)
logger = logging.getLogger("shrimply.server")

with (Path(__file__).parent.parent / "pyproject.toml").open("rb") as file:
    SERVER_VERSION = tomllib.load(file)["project"]["version"]


def read_git_hash() -> str:
    if env.SERVER_GIT_HASH:
        return env.SERVER_GIT_HASH
    try:
        return subprocess.run(
            ["git", "rev-parse", "--verify", "HEAD"],
            cwd=Path(__file__).parent.parent.parent,
            check=True,
            capture_output=True,
            text=True,
            timeout=2,
        ).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


SERVER_GIT_HASH = read_git_hash()


@asynccontextmanager
async def lifespan(_app: Server) -> AsyncGenerator[None]:
    try:
        yield
    finally:
        resource.shutdown_all()


def create_app() -> Server:
    app = Server(
        title="Shrimply Server",
        version=SERVER_VERSION,
        docs_url=None,
        redoc_url=None,
        openapi_url=None,
        lifespan=lifespan,
    )

    @app.get("/")
    def get_status() -> Response:
        return respond(HTTPStatus.OK, server_status(SERVER_VERSION, SERVER_GIT_HASH))

    @app.get("/tts/models")
    def get_tts_models() -> Response:
        return respond(HTTPStatus.OK, ModelsResponse(models=MODELS))

    @app.get("/pneuma/models")
    def get_pneuma_models() -> Response:
        return respond(HTTPStatus.OK, PneumaModelsResponse(models=pneuma_models()))

    @app.get("/video-generation/models")
    def get_video_generation_models() -> Response:
        return respond(
            HTTPStatus.OK,
            VideoGenerationModelsResponse(models=VIDEO_GENERATION_MODELS),
        )

    @app.put("/compute/device")
    def select_compute_device(request: Request) -> Response:
        log_request(request, "compute-device selection")
        device_values = request.query_params.getlist("device")
        if len(device_values) != 1:
            return respond_error(
                HTTPStatus.BAD_REQUEST,
                "invalid_device",
                "Exactly one compute device is required",
            )
        try:
            gpu.select_device(device_values[0])
        except ValueError as exception:
            return respond_error(
                HTTPStatus.BAD_REQUEST, "invalid_device", str(exception)
            )
        except BlockingIOError as exception:
            return respond_error(HTTPStatus.CONFLICT, "compute_busy", str(exception))
        return respond(HTTPStatus.OK, server_status(SERVER_VERSION, SERVER_GIT_HASH))

    @app.put("/compute/jobs/{job_id}/heartbeat")
    def heartbeat_compute_job(job_id: str) -> Response:
        if not resource.heartbeat(job_id):
            return respond_error(
                HTTPStatus.NOT_FOUND, "job_not_found", "Compute job was not found"
            )
        return Response(status_code=HTTPStatus.NO_CONTENT)

    @app.delete("/compute/jobs/{job_id}")
    def cancel_compute_job(job_id: str) -> Response:
        resource.cancel(job_id)
        return Response(status_code=HTTPStatus.NO_CONTENT)

    app.add_api_route("/transcriptions", handle_stt_request, methods=["POST"])
    app.add_api_route("/speech", handle_tts_request, methods=["POST"])
    app.add_api_route("/pneuma/conversions", handle_pneuma_request, methods=["POST"])
    app.add_api_route(
        "/video-generations", handle_video_generation_request, methods=["POST"]
    )
    app.add_api_route("/sam2/analyses", handle_sam2_request, methods=["POST"])
    app.add_api_route(
        "/3dtracking/analyses",
        handle_tracking_3d_request,
        methods=["POST"],
    )
    return app


def main(host: str, port: int, share: bool) -> None:
    logger.info("Shrimply server listening on http://%s:%d", host, port)
    create_app().launch(
        server_name=host,
        server_port=port,
        share=share,
    )


if __name__ == "__main__":
    main(env.SERVER_HOST, env.SERVER_PORT, env.SERVER_SHARE)
