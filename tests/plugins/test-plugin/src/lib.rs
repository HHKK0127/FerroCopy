//! Test WASM plugin for FerroCopy WASI sandbox.
//! Exports `_start` and `filter_file` for testing.
//! This is a #![no_std] plugin — no standard library required.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> i32 {
    0 // success
}

/// Filter function: returns 0 (reject) or 1 (accept).
/// Used to test the wasi_sandbox::WasiPlugin.invoke() path.
#[no_mangle]
pub extern "C" fn filter_file() -> i32 {
    1 // accept all
}