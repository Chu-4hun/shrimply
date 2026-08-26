import torch

device = "cuda:0" if torch.cuda.is_available() else "cpu"


def available_vram() -> int:
    free, _total = torch.cuda.mem_get_info(device)
    return free


def total_vram() -> int:
    return torch.cuda.get_device_properties(device).total_memory


def bf16_supported() -> bool:
    if not device.startswith("cuda:"):
        return False
    with torch.cuda.device(device):
        return torch.cuda.is_bf16_supported()


def is_out_of_memory(message: str) -> bool:
    lowered = message.lower()
    return "out of memory" in lowered or "not enough gpu memory" in lowered


def select_device(selected: str) -> None:
    from api import resource

    available = ("cpu",) + tuple(
        f"cuda:{index}" for index in range(torch.cuda.device_count())
    )
    if selected not in available:
        raise ValueError(f"Compute device {selected!r} is unavailable")
    resource.scheduler.select_device(selected)
