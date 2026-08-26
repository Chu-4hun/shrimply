import logging
import multiprocessing
from collections.abc import Callable
from dataclasses import dataclass
from multiprocessing.connection import Connection
from multiprocessing.process import BaseProcess

from api import gpu, resource
from api.sam2 import resources as sam2_resources
from api.sam2.protocol import (
    ErrorEvent,
    MaskEvent,
    ModelId,
    ProgressEvent,
    ResultEvent,
    WorkerEvent,
    decode_event,
)

logger = logging.getLogger("shrimply.sam2")
spawn_context = multiprocessing.get_context("spawn")
def run_worker(connection: Connection, model_id: ModelId, device: str) -> None:
    from api.sam2.worker import run

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
        name=f"sam2:{model_id}",
    )
    try:
        process.start()
    except Exception:
        parent.close()
        child.close()
        raise
    child.close()
    logger.info("Started SAM2 worker pid=%s model=%s", process.pid, model_id)
    return Worker(key, profile, process, parent)


def analyze(
    job: resource.ManagedJob,
    model_id: ModelId,
    path: str,
    send_event: Callable[[WorkerEvent | resource.QueuedEvent], None],
) -> None:
    key = resource.WorkerKey("sam2", model_id)
    profile = sam2_resources.profile(model_id)
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
                raise RuntimeError("Invalid SAM2 worker")
            connection = worker.connection
            lease.set_state("loading" if worker.state == "loading" else "running")
            send_event(
                ProgressEvent(
                    message=(
                        f"Loading {model_id}..."
                        if worker.state == "loading"
                        else "Analyzing with loaded SAM2 model..."
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
                        raise EOFError("SAM2 worker exited without a result")
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
                    case MaskEvent():
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
