from collections.abc import Generator
from contextlib import contextmanager
from time import perf_counter_ns

import torch


type StageTimings = dict[str, int]


@contextmanager
def measure_stage(
    timings: StageTimings | None,
    name: str,
    device: torch.device,
) -> Generator[None]:
    if timings is None:
        yield
        return
    if device.type == "cuda":
        torch.cuda.synchronize(device)
    started = perf_counter_ns()
    yield
    if device.type == "cuda":
        torch.cuda.synchronize(device)
    timings[name] = timings.get(name, 0) + perf_counter_ns() - started
