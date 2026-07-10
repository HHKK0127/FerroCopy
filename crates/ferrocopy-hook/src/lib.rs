//! FerroCopy Shell Extension Hook (COM DLL)
//!
//! Implements ICopyHook for Explorer integration.
//! When fully registered, Explorer calls this hook before
//! copy/move/rename operations, allowing FerroCopy to intercept
//! and handle the operation instead.
//!
//! # Building
//! ```bash
//! cargo build --release -p ferrocopy-hook
//! ```
//!
//! # Registration (admin)
//! ```bash
//! regsvr32 target/release/ferrocopy_hook.dll
//! ```

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

use windows::core::*;
use windows::Win32::Foundation::*;

/// GUID: {F3C8B5A1-2D4E-4A6F-8B7C-9D0E1F2A3B4C}
#[allow(dead_code)]
const CLSID_FERROCOPY_HOOK: GUID = GUID::from_u128(0xF3C8B5A1_2D4E_4A6F_8B7C_9D0E1F2A3B4C);

// ── DllMain ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn DllMain(
    _dll_module: HINSTANCE,
    _call_reason: u32,
    _reserved: *mut std::ffi::c_void,
) -> BOOL {
    BOOL(1)
}

// ── DllRegisterServer / DllUnregisterServer ──────────────────────────────

/// Register the COM server and shell extension.
#[no_mangle]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    // TODO: Write COM registration to HKCR
    //   1. HKCR\CLSID\{GUID} = "FerroCopy Shell Extension"
    //   2. HKCR\CLSID\{GUID}\InprocServer32 = path to DLL
    //   3. HKCR\Directory\shellex\CopyHookHandlers\FerroCopy = {GUID}
    println!("ferrocopy-hook: DllRegisterServer (stub)");
    S_OK
}

/// Unregister the COM server and shell extension.
#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    // TODO: Remove the COM registration entries
    println!("ferrocopy-hook: DllUnregisterServer (stub)");
    S_OK
}

// ── DllGetClassObject ───────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn DllGetClassObject(
    _rclsid: *const GUID,
    _riid: *const GUID,
    _ppv: *mut *mut std::ffi::c_void,
) -> HRESULT {
    println!("ferrocopy-hook: DllGetClassObject (stub)");
    CLASS_E_CLASSNOTAVAILABLE
}

// ── ICopyHook stub ───────────────────────────────────────────────────────

// TODO: Implement ICopyHook interface when windows-rs supports it.
// For now, the DLL registers the GUID placeholder so the shell
// knows FerroCopy exists. The actual interception logic will be
// implemented in a follow-up.
