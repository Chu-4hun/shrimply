import logging
import multiprocessing
from collections.abc import Callable
from dataclasses import dataclass
from multiprocessing.connection import Connection
from multiprocessing.process import BaseProcess

from api import gpu, resource
from api.tracking_3d import resources as tracking_resources
from api.tracking_3d.protocol import (
    CameraEvent,
    ErrorEvent,
    ModelId,
    ProgressEvent,
    ResultEvent,
    WorkerEvent,
    decode_event,
)

logger = logging.getLogger("shrimply.3dtracking")
spawn_context = multiprocessing.get_context("spawn")
def cuda_selected() -> bool:
    return gpu.device.startswith("cuda:")


def run_worker(connection: Connection, model_id: ModelId, device: str) -> None:
    from api.tracking_3d.worker import run

    run(connection, model_id, device)


@dataclass(slots=True)
class Worker:
    key: resource.WorkerKey
    profile: resource.ResourceProfile
    process: BaseProcess
    connection: Connection
    state: resource.WorkerState = "loading"
    reusable: bool = True

    def healthy(self) -> bool:
        return self.process.is_alive() and not self.connection.closed

    def force_stop(self) -> None:
        resource.force_stop_process(self.process, self.connection)
        self.state = "stopped"


def start_worker(
    key: resource.WorkerKey, profile: resource.ResourceProfile, model_id: ModelId
) -> Worker:
    parent, child = spawn_context.Pipe()
    process = spawn_context.Process(
        target=run_worker,
        args=(child, model_id, gpu.device),
        name=f"3dtracking:{model_id}",
    )
    try:
        process.start()
    except Exception:
        parent.close()
        child.close()
        raise
    child.close()
    logger.info("Started 3D tracking worker pid=%s model=%s", process.pid, model_id)
    return Worker(key, profile, process, parent)


def analyze(
    job: resource.ManagedJob,
    model_id: ModelId,
    path: str,
    send_event: Callable[[WorkerEvent | resource.QueuedEvent], None],
) -> None:
    key = resource.WorkerKey("3dtracking", model_id)
    profile = tracking_resources.profile(model_id)
    lease = resource.scheduler.acquire(
        job,
        key,
        profile,
        lambda: start_worker(key, profile, model_id),
        send_event,
    )
    completed = False
    retried = False
    try:
        while True:
            worker = lease.worker
            if not isinstance(worker, Worker):
                raise RuntimeError("Invalid 3D tracking worker")
            connection = worker.connection
            loading = worker.state == "loading"
            lease.set_state("loading" if loading else "running")
            send_event(
                ProgressEvent(
                    message=(
                        f"Loading {model_id}..."
                        if loading
                        else "Analyzing with loaded 3D tracking model..."
                    ),
                    completed_frames=0,
                    total_frames=1,
                )
            )
            connection.send_bytes(path.encode())
            while True:
                job.check_cancelled()
                if not connection.poll(resource.WORKER_POLL_SECONDS):
                    if not worker.healthy():
                        raise EOFError("3D tracking worker exited without a result")
                    continue
                event = decode_event(connection.recv_bytes())
                match event:
                    case ProgressEvent():
                        lease.set_state(
                            "loading"
                            if event.message.startswith("Loading ")
                            else "running"
                        )
                        send_event(event)
                    case CameraEvent():
                        lease.set_state("running")
                        send_event(event)
                    case ResultEvent():
                        lease.complete()
                        completed = True
                        send_event(event)
                        return
                    case ErrorEvent():
                        if not retried and gpu.is_out_of_memory(event.message):
                            retried = True
                            lease = lease.retry_after_oom()
                            send_event(
                                ProgressEvent(
                                    message="Retrying after freeing GPU memory...",
                                    completed_frames=0,
                                    total_frames=1,
                                )
                            )
                            break
                        send_event(event)
                        return
    finally:
        if not completed:
            lease.discard()
