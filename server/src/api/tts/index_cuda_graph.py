from collections import OrderedDict
from collections.abc import Callable

import torch
from torch import Tensor


type GraphKey = tuple[tuple[tuple[int, ...], torch.dtype, torch.device], ...]
type CapturedGraph = tuple[torch.cuda.CUDAGraph, tuple[Tensor, ...], Tensor]
type CudaGraphCache = OrderedDict[GraphKey, CapturedGraph]

CUDA_GRAPH_CACHE_ENTRIES = 2
CUDA_GRAPH_WARMUPS = 1


def replay_cuda_graph(
    cache: CudaGraphCache,
    function: Callable[..., Tensor],
    inputs: tuple[Tensor, ...],
    pool=None,
) -> Tensor:
    if not inputs or any(value.device.type != "cuda" for value in inputs):
        raise RuntimeError("IndexTTS CUDA Graph inputs must be CUDA tensors")
    device = inputs[0].device
    if any(value.device != device for value in inputs):
        raise RuntimeError("IndexTTS CUDA Graph inputs must share one device")
    key = tuple((tuple(value.shape), value.dtype, value.device) for value in inputs)
    captured = cache.get(key)
    if captured is None:
        static_inputs = tuple(value.detach().clone() for value in inputs)
        capture_stream = torch.cuda.Stream(device=device)
        capture_stream.wait_stream(torch.cuda.current_stream(device))
        with torch.cuda.stream(capture_stream):
            for _ in range(CUDA_GRAPH_WARMUPS):
                function(*static_inputs)
        capture_stream.synchronize()
        graph = torch.cuda.CUDAGraph()
        with torch.cuda.graph(graph, pool=pool, stream=capture_stream):
            static_output = function(*static_inputs)
        captured = graph, static_inputs, static_output
        cache[key] = captured
        cache.move_to_end(key)
        if len(cache) > CUDA_GRAPH_CACHE_ENTRIES:
            _, (old_graph, _, _) = cache.popitem(last=False)
            old_graph.reset()
    else:
        cache.move_to_end(key)
    graph, static_inputs, static_output = captured
    for static, value in zip(static_inputs, inputs, strict=True):
        static.copy_(value)
    graph.replay()
    return static_output
