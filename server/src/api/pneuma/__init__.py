import logging
import multiprocessing
from collections.abc import Callable
from dataclasses import dataclass
from multiprocessing.connection import Connection
from multiprocessing.process import BaseProcess

import msgspec

from api import gpu, resource
from api.pneuma import resources as pneuma_resources
from api.pneuma.catalog import MODEL_DIRECTORY
from api.pneuma.protocol import (
    ConversionRequest,
    ErrorEvent,
    ProgressEvent,
    ResultEvent,
    WorkerEvent,
    WORKER_EVENT_VALIDATOR,
)

logger = logging.getLogger("shrimply.pneuma")
spawn_context = multiprocessing.get_context("spawn")


def run_worker(connection: Connection, device: str) -> None:
    from api.pneuma.worker import run

    run(connection, device)


@dataclass(slots=True)
class PneumaWorker:
    key: resource.WorkerKey
    profile: resource.ResourceProfile
    process: BaseProcess
    connection: Connection
    state: resource.WorkerState = "loading"
    reusable: bool = True

    def healthy(self) -> bool:
        return self.process.is_alive() and not self.connection.closed

    def force_stop(self) -> None:
        pid = self.process.pid
        resource.force_stop_process(self.process, self.connection)
        self.state = "stopped"
        logger.info(
            "Stopped Pneuma worker pid=%s exit_code=%s",
            pid,
            self.process.exitcode,
        )


def start_worker(
    key: resource.WorkerKey, profile: resource.ResourceProfile
) -> PneumaWorker:
    parent_connection, child_connection = spawn_context.Pipe()
    process = spawn_context.Process(
        target=run_worker,
        args=(child_connection, gpu.device),
        name=f"pneuma:{key.model}",
    )
    try:
        process.start()
    except Exception:
        parent_connection.close()
        child_connection.close()
        raise
    child_connection.close()
    logger.info("Started Pneuma worker pid=%s model=%s", process.pid, key.model)
    return PneumaWorker(key, profile, process, parent_connection)


def _receive(worker: PneumaWorker, job: resource.ManagedJob) -> WorkerEvent:
    while True:
        job.check_cancelled()
        if worker.connection.poll(resource.WORKER_POLL_SECONDS):
            return WORKER_EVENT_VALIDATOR.validate_python(
                msgspec.msgpack.decode(worker.connection.recv_bytes())
            )
        if not worker.healthy():
            raise RuntimeError("Pneuma worker stopped unexpectedly")


def convert(
    request: ConversionRequest,
    job: resource.ManagedJob,
    send_event: Callable[[WorkerEvent | resource.QueuedEvent], None],
) -> None:
    key = resource.WorkerKey("pneuma", request.model)
    profile = pneuma_resources.request(request.model, MODEL_DIRECTORY)
    payload = msgspec.msgpack.encode(request.model_dump(mode="python"))
    lease = resource.scheduler.acquire(
        job,
        key,
        profile,
        lambda: start_worker(key, profile),
        send_event,
    )
    retried_after_oom = False
    try:
        while True:
            worker = lease.worker
            if not isinstance(worker, PneumaWorker):
                raise TypeError("Scheduler returned an invalid Pneuma worker")
            starting = worker.state == "loading"
            send_event(
                ProgressEvent(
                    message="Starting worker..." if starting else "Worker ready",
                    model=request.model,
                    state="loading" if starting else "ready",
                )
            )
            sent_request = False
            loading_model = False
            processing = False
            if not starting:
                job.check_cancelled()
                worker.connection.send_bytes(payload)
                sent_request = True
            retry = False
            while True:
                event = _receive(worker, job)
                match event:
                    case ProgressEvent():
                        send_event(event)
                        loading_model = loading_model or event.state == "loading"
                        if event.state == "ready" and not sent_request:
                            job.check_cancelled()
                            worker.connection.send_bytes(payload)
                            sent_request = True
                        elif (
                            sent_request
                            and not processing
                            and event.state == "converting"
                            and (
                                request.model == "none"
                                or loading_model
                                and event.message.startswith("Extracted F0 with ")
                            )
                        ):
                            lease.set_state("running")
                            processing = True
                    case ResultEvent():
                        lease.complete()
                        send_event(event)
                        logger.info(
                            "Pneuma worker pid=%s model=%s completed request",
                            worker.process.pid,
                            request.model,
                        )
                        return
                    case ErrorEvent():
                        if (
                            not retried_after_oom
                            and gpu.is_out_of_memory(event.message)
                        ):
                            retried_after_oom = True
                            lease = lease.retry_after_oom()
                            send_event(
                                ProgressEvent(
                                    message="Retrying after freeing GPU memory...",
                                    model=request.model,
                                    state="loading",
                                )
                            )
                            retry = True
                            break
                        send_event(event)
                        lease.discard()
                        logger.error(
                            "Pneuma worker pid=%s model=%s failed: %s: %s",
                            worker.process.pid,
                            request.model,
                            event.code,
                            event.message,
                        )
                        return
            if retry:
                continue
    except Exception:
        lease.discard()
        job.check_cancelled()
        raise
