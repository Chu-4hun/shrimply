use std::sync::OnceLock;

use shrimply_math_core::Fraction;
use shrimply_math_media::gib_to_bytes;

mod frame_key;
mod pool;

pub use frame_key::ImageKey;
pub use pool::ImagePool;

const DEFAULT_CPU_BYTES: u64 = 4 * 1024 * 1024 * 1024;

pub fn default_cpu_gib() -> Fraction {
    Fraction::from(4_u8)
}

pub fn global() -> &'static ImagePool {
    static POOL: OnceLock<ImagePool> = OnceLock::new();
    POOL.get_or_init(|| ImagePool::new(DEFAULT_CPU_BYTES))
}

pub fn configure(cpu_gib: Fraction) {
    global().set_maximum_bytes(gib_to_bytes(cpu_gib));
}
