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

    fn state_mut() -> &'static mut Out {
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

    /// `len` bytes at `ptr`, or empty for a null pointer or zero length.
    ///
    /// SAFETY: the host passes buffers it got from `alloc` and filled.
    unsafe fn bytes<'a>(ptr: *const u8, len: u32) -> &'a [u8] {
        if ptr.is_null() || len == 0 {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(ptr, len as usize) }
        }
    }

    /// `render(sprites, palette, state, asset, mode)` of `ART_DESCRIPTION.md` 2.2. Returns a
    /// pointer to the result (valid until the next call) or 0 on error.
    #[no_mangle]
    #[allow(clippy::too_many_arguments)]
    pub extern "C" fn render(
        sprites_ptr: *const u8,
        sprites_len: u32,
        palette_ptr: *const u8,
        palette_len: u32,
        state_ptr: *const u8,
        state_len: u32,
        asset_ptr: *const u8,
        mode: u32,
    ) -> *const u8 {
        // SAFETY: see `bytes`.
        let (sprites, palette, state, asset) = unsafe {
            (
                bytes(sprites_ptr, sprites_len),
                bytes(palette_ptr, palette_len),
                bytes(state_ptr, state_len),
                bytes(asset_ptr, deadroll_render::ASSET_LEN as u32),
            )
        };
        let s = state_mut();
        s.bytes = Vec::new();
        let result = match (
            <&[u8; deadroll_render::ASSET_LEN]>::try_from(asset),
            deadroll_render::OutputMode::from_u32(mode),
        ) {
            (Ok(asset), Ok(mode)) => deadroll_render::render(sprites, palette, state, asset, mode),
            (_, Err(e)) => Err(e),
            (Err(_), _) => Err(deadroll_render::Error::Request),
        };
        match result {
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
        state_mut().bytes.len() as u32
    }

    #[no_mangle]
    pub extern "C" fn result_width() -> u32 {
        state_mut().width
    }

    #[no_mangle]
    pub extern "C" fn result_height() -> u32 {
        state_mut().height
    }

    #[no_mangle]
    pub extern "C" fn last_error() -> u32 {
        state_mut().error
    }
}
