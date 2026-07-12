//! FerroCopy COM Shell Extension DLL
//!
//! Provides:
//!   1. ICopyHookW — intercepts Windows Explorer copy/move/rename/delete
//!   2. IContextMenu — right-click context menu (Win10)
//!   3. IExplorerCommand — modern context menu (Win11)
//!   4. IClassFactory — COM server factory
//!   5. Self-registration via DllRegisterServer / DllUnregisterServer

#![allow(non_snake_case, non_camel_case_types)]

use std::sync::atomic::{AtomicU32, Ordering};
use windows_core::implement;
use windows_core::IUnknown;
use windows_core::Interface;
use windows_core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Ole::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::WindowsAndMessaging::*;

// ── Constants ─────────────────────────────────────────────────────────

const CLSID_FERROCOPY_HOOK: windows::core::GUID =
    windows::core::GUID::from_u128(0xF3C8B5A1_2D4E_4A6F_8B7C_9D0E1F2A3B4C);
const REG_NAME: &str = "FerroCopy Shell Extension";

static DLL_REF_COUNT: AtomicU32 = AtomicU32::new(0);

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn alloc_wide(s: &str) -> *mut u16 {
    let wide = to_wide(s);
    let ptr = unsafe { CoTaskMemAlloc(wide.len() * 2) } as *mut u16;
    if !ptr.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len()); }
    }
    ptr
}

fn pcwstr(s: &str) -> (Vec<u16>, windows::core::PCWSTR) {
    let wide = to_wide(s);
    let p = windows::core::PCWSTR::from_raw(wide.as_ptr());
    (wide, p)
}

fn guid_to_string(g: &windows::core::GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.data1, g.data2, g.data3,
        g.data4[0], g.data4[1], g.data4[2], g.data4[3],
        g.data4[4], g.data4[5], g.data4[6], g.data4[7],
    )
}

// ── ICopyHookW ────────────────────────────────────────────────────────

#[implement(ICopyHookW)]
struct FerroCopyCopyHook;

impl ICopyHookW_Impl for FerroCopyCopyHook_Impl {
    fn CopyCallback(
        &self,
        _hwnd: HWND,
        wfunc: u32,
        _wflags: u32,
        pszsrcfile: &windows::core::PCWSTR,
        _dwsrcattribs: u32,
        pszdestfile: &windows::core::PCWSTR,
        _dwdestattribs: u32,
    ) -> u32 {
        let op = match wfunc { 2 => "COPY", 1 => "MOVE", 3 => "DELETE", 4 => "RENAME", _ => "UNKNOWN" };
        let src = unsafe { pszsrcfile.to_string().unwrap_or_default() };
        let dst = unsafe { pszdestfile.to_string().unwrap_or_default() };
        tracing::debug!("CopyHook: op={op} src=\"{src}\" dst=\"{dst}\" — allowing");
        6 // IDYES
    }
}

// ── IContextMenu ──────────────────────────────────────────────────────

#[implement(IContextMenu, IShellExtInit)]
struct FerroCopyContextMenu {
    data_object: std::cell::RefCell<Option<IDataObject>>,
}

impl IShellExtInit_Impl for FerroCopyContextMenu_Impl {
    fn Initialize(
        &self,
        _pidlfolder: *const ITEMIDLIST,
        pdtobj: Option<&IDataObject>,
        _hkeyprogid: HKEY,
    ) -> windows::core::Result<()> {
        *self.data_object.borrow_mut() = pdtobj.cloned();
        Ok(())
    }
}

impl IContextMenu_Impl for FerroCopyContextMenu_Impl {
    fn QueryContextMenu(
        &self,
        hmenu: HMENU,
        indexmenu: u32,
        idcmdfirst: u32,
        _idcmdlast: u32,
        _uflags: u32,
    ) -> windows::core::Result<()> {
        let copy_txt = "FerroCopyにコピー";
        let move_txt = "FerroCopyに移動";
        unsafe {
            let (_copy_buf, copy_wide) = pcwstr(copy_txt);
            let (_move_buf, move_wide) = pcwstr(move_txt);
            let _ = InsertMenuW(
                hmenu,
                indexmenu,
                MF_BYPOSITION | MF_STRING,
                idcmdfirst as usize,
                copy_wide,
            );
            let _ = InsertMenuW(
                hmenu,
                indexmenu + 1,
                MF_BYPOSITION | MF_STRING,
                (idcmdfirst + 1) as usize,
                move_wide,
            );
        }
        Ok(())
    }

    fn InvokeCommand(&self, pici: *const CMINVOKECOMMANDINFO) -> windows::core::Result<()> {
        let ici = unsafe { &*pici };
        let cmd = ici.lpVerb.0 as isize;
        let files = self.get_selected_files();
        if files.is_empty() { return Ok(()); }
        match cmd {
            0 => launch_ferrocopy("--shell-copy", &files),
            1 => launch_ferrocopy("--shell-move", &files),
            _ => {}
        }
        Ok(())
    }

    fn GetCommandString(
        &self,
        _idcmd: usize,
        _utype: u32,
        _preserved: *const u32,
        _pszname: windows::core::PSTR,
        _cchmax: u32,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

impl FerroCopyContextMenu_Impl {
    fn get_selected_files(&self) -> Vec<String> {
        let mut files = Vec::new();
        let dobj_opt = self.data_object.borrow().clone();
        if let Some(ref dobj) = dobj_opt {
            unsafe {
                let cff = FORMATETC {
                    cfFormat: 15u16,
                    ptd: std::ptr::null_mut(),
                    dwAspect: 1u32, // DVASPECT_CONTENT
                    lindex: -1,
                    tymed: 1u32,    // TYMED_HGLOBAL
                };
                if let Ok(mut medium) = dobj.GetData(&cff as *const FORMATETC) {
                    let hdrop = std::mem::transmute::<_, HDROP>(medium.u.hGlobal);
                    let count = DragQueryFileW(hdrop, u32::MAX, None);
                    for i in 0..count {
                        let len = DragQueryFileW(hdrop, i, None);
                        if len > 0 {
                            let mut buf = vec![0u16; (len + 1) as usize];
                            DragQueryFileW(hdrop, i, Some(&mut buf));
                            files.push(String::from_utf16_lossy(&buf[..len as usize]));
                        }
                    }
                    let _ = ReleaseStgMedium(&mut medium as *mut STGMEDIUM);
                }
            }
        }
        files
    }
}

// ── IExplorerCommand (Win11) ──────────────────────────────────────────

#[implement(IExplorerCommand)]
struct FerroCopyExplorerCommand;

impl IExplorerCommand_Impl for FerroCopyExplorerCommand_Impl {
    fn GetTitle(&self, _psiitemarray: Option<&IShellItemArray>) -> windows::core::Result<windows::core::PWSTR> {
        Ok(windows::core::PWSTR(alloc_wide("FerroCopyにコピー")))
    }

    fn GetIcon(&self, _psiitemarray: Option<&IShellItemArray>) -> windows::core::Result<windows::core::PWSTR> {
        let exe = get_ferrocopy_exe_path().unwrap_or_default();
        Ok(windows::core::PWSTR(alloc_wide(&exe)))
    }

    fn GetToolTip(&self, _psiitemarray: Option<&IShellItemArray>) -> windows::core::Result<windows::core::PWSTR> {
        Ok(windows::core::PWSTR(std::ptr::null_mut()))
    }

    fn GetCanonicalName(&self) -> windows::core::Result<windows::core::GUID> { Ok(CLSID_FERROCOPY_HOOK) }

    fn GetState(&self, _psiitemarray: Option<&IShellItemArray>, _foktobeslow: BOOL) -> windows::core::Result<u32> {
        Ok(0)
    }

    fn Invoke(&self, psiitemarray: Option<&IShellItemArray>, _pbc: Option<&IBindCtx>) -> windows::core::Result<()> {
        if let Some(items) = psiitemarray {
            let mut files = Vec::new();
            unsafe {
                let count = items.GetCount().unwrap_or(0);
                for i in 0..count {
                    if let Ok(item) = items.GetItemAt(i) {
                        if let Ok(path) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                            if let Ok(s) = path.to_string() { files.push(s); }
                        }
                    }
                }
            }
            if !files.is_empty() { launch_ferrocopy("--shell-copy", &files); }
        }
        Ok(())
    }

    fn GetFlags(&self) -> windows::core::Result<u32> { Ok(0) }
    fn EnumSubCommands(&self) -> windows::core::Result<IEnumExplorerCommand> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }
}

// ── IClassFactory ─────────────────────────────────────────────────────

#[implement(IClassFactory)]
struct FerroCopyFactory;

impl IClassFactory_Impl for FerroCopyFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Option<&IUnknown>,
        riid: *const windows::core::GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        unsafe {
            if punkouter.is_some() { return Err(windows::core::Error::from(CLASS_E_NOAGGREGATION)); }
            let guid = &*riid;
            if *guid == <ICopyHookW as Interface>::IID {
                let obj: ICopyHookW = FerroCopyCopyHook.into();
                *ppvobject = std::mem::transmute(obj);
                DLL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
                return Ok(());
            }
            if *guid == <IContextMenu as Interface>::IID {
                            let obj: IContextMenu = FerroCopyContextMenu { data_object: std::cell::RefCell::new(None) }.into();
                *ppvobject = std::mem::transmute(obj);
                DLL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
                return Ok(());
            }
            if *guid == <IExplorerCommand as Interface>::IID {
                let obj: IExplorerCommand = FerroCopyExplorerCommand.into();
                *ppvobject = std::mem::transmute(obj);
                DLL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
                return Ok(());
            }
            Err(windows::core::Error::from(E_NOINTERFACE))
        }
    }

    fn LockServer(&self, _flock: BOOL) -> windows::core::Result<()> { Ok(()) }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn get_ferrocopy_exe_path() -> Option<String> {
    unsafe {
        let mut buf = vec![0u16; (MAX_PATH + 1) as usize];
        let module = GetModuleHandleW(None).ok()?;
        let len = GetModuleFileNameW(module, &mut buf);
        if len > 0 {
            Some(String::from_utf16_lossy(&buf[..len as usize]).replace("ferrocopy_hook.dll", "ferrocopy.exe"))
        } else {
            None
        }
    }
}

fn launch_ferrocopy(action: &str, files: &[String]) {
    let exe = match get_ferrocopy_exe_path() { Some(p) => p, None => return };
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg(action);
    for f in files { cmd.arg(f); }
    if cmd.spawn().is_err() { tracing::error!("Failed to launch ferrocopy.exe"); }
}

fn get_dll_path() -> Option<String> {
    unsafe {
        let mut buf = vec![0u16; (MAX_PATH + 1) as usize];
        let module = GetModuleHandleW(None).ok()?;
        let len = GetModuleFileNameW(module, &mut buf);
        if len > 0 { Some(String::from_utf16_lossy(&buf[..len as usize])) } else { None }
    }
}

// ── Registry helpers ─────────────────────────────────────────────────

unsafe fn set_reg_str(hkey: HKEY, subkey: &str, value: &str, name: Option<&str>) -> bool {
    let mut hk = HKEY::default();
    let mut disp = REG_CREATE_KEY_DISPOSITION::default();
    let (key_buf, key_pw) = pcwstr(subkey);
    let status = RegCreateKeyExW(
        hkey,
        key_pw,
        0,
        PCWSTR::null(),
        REG_OPTION_NON_VOLATILE,
        REG_SAM_FLAGS::default(),
        None,
        &mut hk,
        Some(&mut disp),
    );
    let _ = key_buf;
    if status != WIN32_ERROR(0) { return false; }
    let wide_val = to_wide(value);
    let (name_buf, name_ptr) = match name {
        Some(n) => {
            let b = to_wide(n);
            let p = windows::core::PCWSTR::from_raw(b.as_ptr());
            (Some(b), p)
        }
        None => (None, windows::core::PCWSTR::null()),
    };
    let _ = RegSetValueExW(
        hk,
        name_ptr,
        0,
        REG_SZ,
        Some(wide_val.as_u8_slice()),
    );
    let _ = name_buf;
    let _ = RegCloseKey(hk);
    true
}

unsafe fn delete_reg_tree(hkey: HKEY, path: &str) -> bool {
    let mut subkey = HKEY::default();
    let (path_buf, path_pw) = pcwstr(path);
    let status = RegOpenKeyExW(hkey, path_pw, 0, KEY_READ | KEY_WRITE, &mut subkey);
    let _ = path_buf;
    if status != WIN32_ERROR(0) { return false; }

    loop {
        let mut name_buf = [0u16; 256];
        let mut name_len = 256u32;
        let result = RegEnumKeyExW(
            subkey,
            0,
            windows::core::PWSTR(name_buf.as_mut_ptr()),
            &mut name_len,
            None,
            windows::core::PWSTR::null(),
            None,
            None,
        );
        if result != WIN32_ERROR(0) { break; }
        let sub_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        let _ = delete_reg_tree(hkey, &format!("{}\\{}", path, sub_name));
    }
    let _ = RegCloseKey(subkey);
    let (path_buf2, path_pw2) = pcwstr(path);
    let res = RegDeleteKeyExW(hkey, path_pw2, KEY_WOW64_64KEY.0, 0);
    let _ = path_buf2;
    res == WIN32_ERROR(0)
}

trait U8Slice {
    fn as_u8_slice(&self) -> &[u8];
}
impl U8Slice for [u16] {
    fn as_u8_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * 2) }
    }
}

// ── COM DLL Exports ───────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "system" fn DllMain(_hinst: HINSTANCE, _fdwreason: u32, _lpvreserved: *mut core::ffi::c_void) -> BOOL { BOOL(1) }

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const windows::core::GUID, riid: *const windows::core::GUID, ppv: *mut *mut core::ffi::c_void,
) -> windows::core::HRESULT {
    let clsid = &*rclsid;
    let iid = &*riid;
    if *clsid != CLSID_FERROCOPY_HOOK { return CLASS_E_CLASSNOTAVAILABLE; }
    if *iid != <IClassFactory as Interface>::IID { return E_NOINTERFACE; }
    let factory: IClassFactory = FerroCopyFactory.into();
    *ppv = std::mem::transmute(factory);
    DLL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
    S_OK
}

#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> windows::core::HRESULT {
    let dll_path = match get_dll_path() {
        Some(p) => p,
        None => return windows::core::HRESULT(0x80040201u32 as _),
    };
    let clsid_str = guid_to_string(&CLSID_FERROCOPY_HOOK);
    let hkcr = HKEY_CLASSES_ROOT;

    // CLSID entry
    set_reg_str(hkcr, &format!("CLSID\\{clsid_str}"), REG_NAME, None);
    set_reg_str(hkcr, &format!("CLSID\\{clsid_str}\\InprocServer32"), &dll_path, None);
    set_reg_str(hkcr, &format!("CLSID\\{clsid_str}\\InprocServer32"), "Apartment", Some("ThreadingModel"));

    // Context menu handlers (HKCU, no admin needed)
    for path in [
        "Software\\Classes\\*\\shellex\\ContextMenuHandlers\\FerroCopy",
        "Software\\Classes\\Directory\\shellex\\ContextMenuHandlers\\FerroCopy",
        "Software\\Classes\\AllFilesystemObjects\\shellex\\ContextMenuHandlers\\FerroCopy",
        "Software\\Classes\\Directory\\shellex\\CopyHookHandlers\\FerroCopy",
    ] {
        set_reg_str(HKEY_CURRENT_USER, path, &clsid_str, None);
    }
    S_OK
}

#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> windows::core::HRESULT {
    let clsid_str = guid_to_string(&CLSID_FERROCOPY_HOOK);

    // Remove CLSID tree
    delete_reg_tree(HKEY_CLASSES_ROOT, &format!("CLSID\\{clsid_str}"));

    // Remove context menu handlers
    for path in [
        "Software\\Classes\\*\\shellex\\ContextMenuHandlers\\FerroCopy",
        "Software\\Classes\\Directory\\shellex\\ContextMenuHandlers\\FerroCopy",
        "Software\\Classes\\AllFilesystemObjects\\shellex\\ContextMenuHandlers\\FerroCopy",
        "Software\\Classes\\Directory\\shellex\\CopyHookHandlers\\FerroCopy",
    ] {
        delete_reg_tree(HKEY_CURRENT_USER, path);
    }
    S_OK
}

#[no_mangle]
pub unsafe extern "system" fn DllCanUnloadNow() -> windows::core::HRESULT {
    if DLL_REF_COUNT.load(Ordering::SeqCst) == 0 { S_OK } else { S_FALSE }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_clsid_format() {
        let s = guid_to_string(&CLSID_FERROCOPY_HOOK);
        assert!(s.contains("F3C8B5A1"));
    }
}