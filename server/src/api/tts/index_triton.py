import torch
import triton
import triton.language as tl


def _launch(kernel, grid, *args, **kwargs):
    kernel[grid](*args, **kwargs)


@triton.jit
def _adaptive_rms_norm_kernel(
    values,
    scale,
    bias,
    norm_weight,
    output,
    scale_row_stride,
    bias_row_stride,
    sequence_length,
    dimension: tl.constexpr,
    epsilon: tl.constexpr,
    block_size: tl.constexpr,
):
    row = tl.program_id(0)
    columns = tl.arange(0, block_size)
    mask = columns < dimension
    offsets = row * dimension + columns
    input_values = tl.load(values + offsets, mask=mask, other=0.0)
    values_float = input_values.to(tl.float32)
    inverse_rms = tl.rsqrt(
        tl.sum(values_float * values_float, axis=0) / dimension + epsilon
    )
    parameter_row = row // sequence_length
    projected_scale = tl.load(
        scale + parameter_row * scale_row_stride + columns,
        mask=mask,
        other=0.0,
    )
    projected_bias = tl.load(
        bias + parameter_row * bias_row_stride + columns,
        mask=mask,
        other=0.0,
    )
    base_weight = tl.load(norm_weight + columns, mask=mask, other=0.0)
    normalized = (values_float * inverse_rms).to(input_values.dtype)
    weighted = (normalized * base_weight).to(input_values.dtype)
    scaled = (projected_scale * weighted).to(input_values.dtype)
    result = scaled + projected_bias
    tl.store(output + offsets, result, mask=mask)


@triton.jit
def _snake_beta_kernel(
    values,
    alpha,
    beta,
    output,
    elements,
    channels,
    length,
    logarithmic: tl.constexpr,
    block_size: tl.constexpr,
):
    offsets = tl.program_id(0) * block_size + tl.arange(0, block_size)
    mask = offsets < elements
    channel = (offsets // length) % channels
    value = tl.load(values + offsets, mask=mask, other=0.0)
    alpha_value = tl.load(alpha + channel, mask=mask, other=0.0)
    beta_value = tl.load(beta + channel, mask=mask, other=0.0)
    if logarithmic:
        alpha_value = tl.exp(alpha_value.to(tl.float32)).to(alpha_value.dtype)
        beta_value = tl.exp(beta_value.to(tl.float32)).to(beta_value.dtype)
    argument = (value * alpha_value).to(value.dtype)
    sine = tl.sin(argument.to(tl.float32)).to(value.dtype)
    squared = (sine * sine).to(value.dtype)
    denominator = (beta_value + 1.0e-9).to(value.dtype)
    result = value + (squared / denominator).to(value.dtype)
    tl.store(output + offsets, result, mask=mask)


@triton.jit
def _weight_norm_kernel(
    weight_v,
    weight_g,
    output,
    columns: tl.constexpr,
    block_size: tl.constexpr,
):
    row = tl.program_id(0)
    column_offsets = tl.arange(0, block_size)
    mask = column_offsets < columns
    offsets = row * columns + column_offsets
    values = tl.load(weight_v + offsets, mask=mask, other=0.0)
    values_float = values.to(tl.float32)
    norm = tl.sqrt(tl.sum(values_float * values_float, axis=0)).to(values.dtype)
    product = (values * tl.load(weight_g + row)).to(values.dtype)
    tl.store(output + offsets, product / norm, mask=mask)


def require_triton() -> None:
    if not torch.cuda.is_available():
        raise RuntimeError("IndexTTS Triton kernels require CUDA")


def adaptive_rms_norm(
    values: torch.Tensor,
    scale: torch.Tensor,
    bias: torch.Tensor,
    norm_weight: torch.Tensor,
    epsilon: float,
) -> torch.Tensor:
    if values.device.type != "cuda":
        raise RuntimeError("IndexTTS adaptive RMSNorm requires CUDA")
    if values.ndim != 3 or scale.ndim != 3 or bias.shape != scale.shape:
        raise ValueError("IndexTTS adaptive RMSNorm received invalid tensor shapes")
    dimension = values.shape[-1]
    if scale.shape[-1] != dimension or norm_weight.shape != (dimension,):
        raise ValueError("IndexTTS adaptive RMSNorm dimensions do not match")
    if scale.shape[0] != values.shape[0] or scale.shape[1] != 1:
        raise ValueError("IndexTTS adaptive RMSNorm requires one condition per batch")
    contiguous = values.contiguous()
    output = torch.empty_like(contiguous)
    rows = contiguous.numel() // dimension
    block_size = triton.next_power_of_2(dimension)
    _launch(
        _adaptive_rms_norm_kernel,
        (rows,),
        contiguous,
        scale,
        bias,
        norm_weight,
        output,
        scale.stride(0),
        bias.stride(0),
        values.shape[1],
        dimension,
        epsilon,
        block_size,
        num_warps=8,
    )
    return output


def snake_beta(
    values: torch.Tensor,
    alpha: torch.Tensor,
    beta: torch.Tensor,
    logarithmic: bool,
) -> torch.Tensor:
    if values.device.type != "cuda":
        raise RuntimeError("IndexTTS SnakeBeta requires CUDA")
    if values.ndim != 3 or alpha.ndim != 1 or beta.shape != alpha.shape:
        raise ValueError("IndexTTS SnakeBeta received invalid tensor shapes")
    if values.shape[1] != alpha.shape[0]:
        raise ValueError("IndexTTS SnakeBeta channel count does not match")
    contiguous = values.contiguous()
    output = torch.empty_like(contiguous)
    elements = contiguous.numel()
    block_size = 256
    _launch(
        _snake_beta_kernel,
        (triton.cdiv(elements, block_size),),
        contiguous,
        alpha,
        beta,
        output,
        elements,
        values.shape[1],
        values.shape[2],
        logarithmic,
        block_size,
        num_warps=4,
    )
    return output


def weight_norm(
    weight_v: torch.Tensor,
    weight_g: torch.Tensor,
    *,
    inplace: bool = False,
) -> torch.Tensor:
    if weight_v.device.type != "cuda":
        raise RuntimeError("IndexTTS weight normalization requires CUDA")
    if weight_v.ndim < 2 or weight_g.numel() != weight_v.shape[0]:
        raise ValueError("IndexTTS weight normalization dimensions do not match")
    if inplace and not weight_v.is_contiguous():
        raise ValueError("In-place IndexTTS weight normalization requires contiguous weights")
    contiguous = weight_v if inplace else weight_v.contiguous()
    rows = contiguous.shape[0]
    columns = contiguous.numel() // rows
    block_size = triton.next_power_of_2(columns)
    if block_size > 65_536:
        raise ValueError("IndexTTS weight normalization row is too large for Triton")
    output = contiguous if inplace else torch.empty_like(contiguous)
    _launch(
        _weight_norm_kernel,
        (rows,),
        contiguous,
        weight_g,
        output,
        columns,
        block_size,
        num_warps=4 if block_size <= 2_048 else 8,
    )
    return output
