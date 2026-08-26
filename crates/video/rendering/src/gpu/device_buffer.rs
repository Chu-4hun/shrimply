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
        let ptr = unsafe { memory::malloc_sync(byte_len) }
            .map_err(|error| format!("allocate CUDA params: {error:?}"))?;
        unsafe { DeviceBuffer::from_raw_parts(ptr, data.len(), stream.context().clone()) }
    };
    if let Err(error) =
        unsafe { memory::memcpy_htod_sync(buffer.cu_deviceptr(), data.as_ptr(), byte_len) }
    {
        drop(buffer);
        return Err(format!("copy CUDA params: {error:?}"));
    }
    Ok(buffer)
}
