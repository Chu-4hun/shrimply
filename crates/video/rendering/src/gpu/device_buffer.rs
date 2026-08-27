use std::mem;

use cuda_core::{CudaContext, CudaStream, DeviceBuffer, memory};

pub(super) fn copy<T>(
    context: &CudaContext,
    stream: &CudaStream,
    data: &[T],
    spare: Option<DeviceBuffer<T>>,
) -> Result<DeviceBuffer<T>, String> {
    let byte_len = mem::size_of_val(data);
    if byte_len == 0 {
        return Ok(unsafe {
            DeviceBuffer::from_raw_parts(0, data.len(), stream.context().clone())
        });
    }

    context
        .bind_to_thread()
        .map_err(|error| format!("bind CUDA context for params upload: {error:?}"))?;
    let buffer = if let Some(buffer) = spare.filter(|buffer| buffer.len() == data.len()) {
        buffer
    } else {
        shrimply_gpu_memory::global()
            .allocate_buffer::<u8>(
                stream,
                byte_len,
                shrimply_gpu_memory::AllocationClass::Transient,
                "CUDA compositor parameters",
            )?
            .cast_chunks::<T>()
            .map_err(|_| "CUDA compositor parameter buffer alignment mismatch".to_string())?
    };
    // New buffers are zeroed asynchronously on this stream. Order that memset
    // (and any previous use of a recycled buffer) before the legacy synchronous
    // host upload, which is not ordered with a non-blocking CUDA stream.
    if let Err(error) = stream.synchronize() {
        drop(buffer);
        return Err(format!("synchronize CUDA params buffer: {error:?}"));
    }
    if let Err(error) =
        unsafe { memory::memcpy_htod_sync(buffer.cu_deviceptr(), data.as_ptr(), byte_len) }
    {
        drop(buffer);
        return Err(format!("copy CUDA params: {error:?}"));
    }
    Ok(buffer)
}
