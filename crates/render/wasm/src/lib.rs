//! WASM C ABI of the rasterizer (`crates/render/FORMAT.md` 7). No `wasm-bindgen`: the same module
//! runs in the browser, a worker and Node. Build with the `wasm` profile, then `wasm-opt -Oz`.
//! On the host this crate compiles to nothing.
#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(target_arch = "wasm32")]
mod abi {
    extern crate alloc;

    use alloc::alloc::{alloc as raw_alloc, dealloc as raw_dealloc, Layout};
    use alloc::vec::Vec;
    use core::cell::UnsafeCell;

    #[global_allocator]
    static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

    #[panic_handler]
    fn panic(_: &core::panic::PanicInfo) -> ! {
        core::arch::wasm32::unreachable()
    }

    struct Out {
        bytes: Vec<u8>,
        width: u32,
        height: u32,
        error: u32,
    }

    /// Module state. WASM here is single-threaded, so one global cell is sound.
    struct State(UnsafeCell<Out>);
    unsafe impl Sync for State {}

    static STATE: State = State(UnsafeCell::new(Out {
        bytes: Vec::new(),
        width: 0,
        height: 0,
        error: 0,
    }));

    fn state() -> &'static mut Out {
        // SAFETY: single-threaded module; no reference outlives one exported call.
        unsafe { &mut *STATE.0.get() }
    }

    #[no_mangle]
    pub extern "C" fn alloc(len: u32) -> *mut u8 {
        match Layout::from_size_align(len as usize, 1) {
            Ok(l) if len > 0 => unsafe { raw_alloc(l) },
            _ => core::ptr::null_mut(),
        }
    }

    #[no_mangle]
    pub extern "C" fn dealloc(ptr: *mut u8, len: u32) {
        if ptr.is_null() || len == 0 {
            return;
        }
        if let Ok(l) = Layout::from_size_align(len as usize, 1) {
            unsafe { raw_dealloc(ptr, l) }
        }
    }

    /// Renders; returns a pointer to the result (valid until the next call) or 0 on error.
    #[no_mangle]
    pub extern "C" fn render(
        input_ptr: *const u8,
        input_len: u32,
        out_mode: u32,
        scale: u32,
    ) -> *const u8 {
        let input: &[u8] = if input_ptr.is_null() || input_len == 0 {
            &[]
        } else {
            // SAFETY: the host passes a buffer it got from `alloc` and filled.
            unsafe { core::slice::from_raw_parts(input_ptr, input_len as usize) }
        };
        let s = state();
        s.bytes = Vec::new();
        match deadroll_render::render(input, out_mode, scale) {
            Ok(out) => {
                *s = Out {
                    bytes: out.bytes,
                    width: out.width,
                    height: out.height,
                    error: 0,
                };
                s.bytes.as_ptr()
            }
            Err(e) => {
                *s = Out {
                    bytes: Vec::new(),
                    width: 0,
                    height: 0,
                    error: e.code(),
                };
                core::ptr::null()
            }
        }
    }

    #[no_mangle]
    pub extern "C" fn result_len() -> u32 {
        state().bytes.len() as u32
    }

    #[no_mangle]
    pub extern "C" fn result_width() -> u32 {
        state().width
    }

    #[no_mangle]
    pub extern "C" fn result_height() -> u32 {
        state().height
    }

    #[no_mangle]
    pub extern "C" fn last_error() -> u32 {
        state().error
    }
}
