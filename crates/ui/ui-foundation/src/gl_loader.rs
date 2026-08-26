use std::ffi::{CString, c_void};

#[link(name = "GL")]
unsafe extern "C" {
    fn glXGetProcAddressARB(proc_name: *const u8) -> *const c_void;
}

pub fn proc_address(symbol: &str) -> *const c_void {
    let Ok(symbol) = CString::new(symbol) else {
        return std::ptr::null();
    };
    unsafe { glXGetProcAddressARB(symbol.as_ptr().cast()) }
}

pub fn context() -> glow::Context {
    unsafe { glow::Context::from_loader_function(proc_address) }
}
