import logging
import multiprocessing
import os
import threading
from collections.abc import Callable
from dataclasses import dataclass, field, replace
from multiprocessing.connection import Connection
from multiprocessing.process import BaseProcess
from pathlib import Path

import msgspec

import env

from api import gpu, resource
from api.video_generation import resources as generation_resources
from api.video_generation.minimax_h3.inference import GenerationRequest as H3Request
from api.video_generation.protocol import (
    ErrorEvent,
    GenerationRequest,
    ModelId,
    OutputChunkEvent,
    OutputStartEvent,
    ProgressEvent,
    PublicEvent,
    ResultEvent,
    WORKER_EVENT_VALIDATOR,
    WorkerCheckpointEvent,
    WorkerResultEvent,
    WorkerStage,
)
from api.video_generation.requests import PreparedRequest, prepare_request
from api.video_generation.wan.inference import GenerationRequest as WanRequest

logger = logging.getLogger("shrimply.video_generation")
spawn_context = multiprocessing.get_context("spawn")
OUTPUT_CHUNK_BYTES = 1024 * 1024


def run_worker(
    connection: Connection,
    request: PreparedRequest,
    model_id: ModelId,
    device: str,
    stage: WorkerStage,
) -> None:
    from api.video_generation.worker import run

    run(connection, request, model_id, device, stage)


def spawn_worker(
    request: PreparedRequest, model_id: ModelId, stage: WorkerStage
) -> tuple[BaseProcess, Connection]:
    parent, child = spawn_context.Pipe()
    process = spawn_context.Process(
        target=run_worker,
        args=(child, request, model_id, gpu.device, stage),
        name=f"video-generation:{stage}:{model_id}",
    )
    try:
        process.start()
    except Exception:
        parent.close()
        child.close()
        raise
    child.close()
    logger.info(
        "Started video-generation worker pid=%s model=%s stage=%s",
        process.pid,
        model_id,
        stage,
    )
    return process, parent


@dataclass(slots=True)
class Worker:
    key: resource.WorkerKey
    profile: resource.ResourceProfile
    request: PreparedRequest
    model_id: ModelId
    process: BaseProcess | None
    connection: Connection | None
    state: resource.WorkerState = "loading"
    reusable: bool = False
    stopped: bool = False
    lock: threading.Lock = field(default_factory=threading.Lock)

    def healthy(self) -> bool:
        with self.lock:
            return (
                not self.stopped
                and self.process is not None
                and self.process.is_alive()
                and self.connection is not None
                and not self.connection.closed
            )

    def replace_process(self, stage: WorkerStage) -> Connection:
        with self.lock:
            if self.stopped:
                raise RuntimeError("Video-generation worker was cancelled")
            process, self.process = self.process, None
            connection, self.connection = self.connection, None
        resource.force_stop_process(process, connection)
        process, connection = spawn_worker(self.request, self.model_id, stage)
        with self.lock:
            if self.stopped:
                cancelled = True
            else:
                self.process = process
                self.connection = connection
                self.state = "loading"
                cancelled = False
        if cancelled:
            resource.force_stop_process(process, connection)
            raise RuntimeError("Video-generation worker was cancelled")
        return connection

    def force_stop(self) -> None:
        with self.lock:
            self.stopped = True
            process, self.process = self.process, None
            connection, self.connection = self.connection, None
            self.state = "stopped"
        resource.force_stop_process(process, connection)


def start_worker(
    key: resource.WorkerKey,
    profile: resource.ResourceProfile,
    request: PreparedRequest,
    model_id: ModelId,
    stage: WorkerStage,
) -> Worker:
    process, connection = spawn_worker(request, model_id, stage)
    return Worker(key, profile, request, model_id, process, connection)


def worker_key(model_id: ModelId, request: PreparedRequest) -> resource.WorkerKey:
    if isinstance(request, WanRequest):
        configuration = {
            "pipeline": "wan",
            "workflow": request.workflow,
        }
    else:
        configuration = {
            "adapter": request.lora or "",
            "adapter_scale": str(request.lora_scale),
            "adapter_weight": request.lora_weight_name or "",
            "attention": request.attention,
            "memory": request.memory,
            "pipeline": "minimax_h3",
            "quantization": "int8" if request.memory == "int8" else "bf16",
            "workflow": request.workflow,
        }
    return resource.WorkerKey(
        "video_generation", model_id, tuple(sorted(configuration.items()))
    )


def with_transient_output(
    request: PreparedRequest, job_id: str
) -> tuple[PreparedRequest, Path, Path]:
    final_output = request.output
    transient = final_output.with_name(
        f".{final_output.stem}.{job_id}{final_output.suffix}"
    )
    if isinstance(request, H3Request):
        return replace(request, output=transient), final_output, transient
    return request.model_copy(update={"output": transient}), final_output, transient


def remove_transient_output(path: Path) -> None:
    path.unlink(missing_ok=True)
    path.with_name(path.name + ".tmp").unlink(missing_ok=True)
    path.with_name(f".{path.stem}.partial{path.suffix}").unlink(missing_ok=True)


def stream_output(
    job: resource.ManagedJob,
    path: Path,
    send_event: Callable[[PublicEvent | resource.QueuedEvent], None],
) -> None:
    size = path.stat().st_size
    if size <= 0:
        raise RuntimeError("Video generation produced an empty output")
    send_event(OutputStartEvent(bytes=size))
    with path.open("rb") as output:
        while chunk := output.read(OUTPUT_CHUNK_BYTES):
            job.check_cancelled()
            send_event(OutputChunkEvent(data=chunk))


def generate(
    job: resource.ManagedJob,
    request: GenerationRequest,
    send_event: Callable[[PublicEvent | resource.QueuedEvent], None],
) -> None:
    if not gpu.device.startswith("cuda:"):
        raise RuntimeError("Video generation requires a selected CUDA device")
    prepared = prepare_request(request)
    request.inputs.clear()
    prepared, final_output, transient_output = with_transient_output(prepared, job.id)
    profile = generation_resources.profile(prepared)
    key = worker_key(request.model, prepared)
    env.configure_video_generation_worker()
    stage: WorkerStage = "generation"
    lease = resource.scheduler.acquire(
        job,
        key,
        profile,
        lambda: start_worker(key, profile, prepared, request.model, stage),
        send_event,
    )
    completed = False
    committed = False
    retried = False
    try:
        while True:
            worker = lease.worker
            if not isinstance(worker, Worker) or worker.connection is None:
                raise RuntimeError("Invalid video-generation worker")
            connection = worker.connection
            progress = ProgressEvent(
                message=(
                    "Starting video-generation worker…"
                    if stage == "generation"
                    else "Starting isolated MiniMax H3 decode worker…"
                ),
                model=request.model,
                state="loading",
            )
            lease.set_state("loading")
            send_event(progress)
            restart = False
            while True:
                job.check_cancelled()
                if not connection.poll(resource.WORKER_POLL_SECONDS):
                    if not worker.healthy():
                        raise EOFError(
                            "Video-generation worker exited without a result"
                        )
                    continue
                event = WORKER_EVENT_VALIDATOR.validate_python(
                    msgspec.msgpack.decode(connection.recv_bytes())
                )
                match event:
                    case ProgressEvent():
                        progress = event
                        state: resource.WorkerState = (
                            "loading"
                            if event.state == "loading"
                            else "decoding"
                            if event.state == "decoding"
                            else "streaming"
                            if event.state == "streaming"
                            else "running"
                        )
                        lease.set_state(state)
                        send_event(event)
                    case WorkerCheckpointEvent():
                        job.check_cancelled()
                        stage = "decode"
                        connection = worker.replace_process(stage)
                        progress = ProgressEvent(
                            message="Starting isolated MiniMax H3 decode worker…",
                            model=request.model,
                            state="loading",
                        )
                        lease.set_state("loading")
                        send_event(progress)
                    case WorkerResultEvent():
                        job.check_cancelled()
                        output = Path(event.output)
                        if output != transient_output:
                            raise RuntimeError(
                                "Worker returned an unexpected output path"
                            )
                        os.replace(transient_output, final_output)
                        committed = True
                        lease.set_state("streaming")
                        send_event(
                            ProgressEvent(
                                message="Transferring generated video…",
                                model=request.model,
                                state="streaming",
                            )
                        )
                        stream_output(job, final_output, send_event)
                        lease.complete()
                        completed = True
                        send_event(ResultEvent(result=event.result))
                        return
                    case ErrorEvent():
                        if not retried and gpu.is_out_of_memory(event.message):
                            retried = True
                            lease = lease.retry_after_oom()
                            send_event(
                                ProgressEvent(
                                    message="Retrying after freeing GPU memory…",
                                    model=request.model,
                                    state="loading",
                                )
                            )
                            restart = True
                            break
                        send_event(event)
                        return
            if not restart:
                return
    finally:
        if not completed:
            lease.discard()
        if not committed:
            remove_transient_output(transient_output)
