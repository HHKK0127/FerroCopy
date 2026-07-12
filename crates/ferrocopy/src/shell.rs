//! FerroCopy Windows Shell Integration
//!
//! Provides:
//!   1. Right-click context menu "Copy with FerroCopy" / "Move with FerroCopy"
//!   2. Send To entry
//!   3. Explorer copy handler registration (placeholder for COM plugin)
//!
//! All registrations target HKEY_CURRENT_USER (no admin required).
//!
//! 【OS既存コピー差し替えアーキテクチャ】
//!
//! 完全な差し替えには COM DLL シェル拡張が必要ですが、
//! 本モジュールでは以下の段階的アプローチを採用します:
//!
//!   Phase 1 (本実装): 右クリックコンテキストメニュー
//!     ・ファイル/フォルダ右クリック → "FerroCopyにコピー" / "FerroCopyに移動"
//!     ・フォルダ背景右クリック → "FerroCopyでペースト"
//!     ・Send To メニュー → "FerroCopy"
//!
//!   Phase 2 (将来): IFileOperation フック COM DLL
//!     ・Windows Explorer の Ctrl+C/V をインターセプト
//!     ・全てのファイル操作を FerroCopy にリダイレクト
//!     ・別 crate「ferrocopy-hook」として cdylib ビルド
//!
//!   Phase 3 (将来): IExplorerCommand によるモダンコンテキストメニュー
//!     ・Windows 11 スタイルのポップアップメニュー
//!     ・アイコン表示、ピン留め、サブメニュー
//!

use anyhow::{Context, Result};
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;
use crate::gui;

/// Recursively delete a registry key tree using winreg.
/// winreg 0.52 does not have delete_subkey_tree, so we implement it manually.
fn delete_reg_tree(hkcu: &RegKey, path: &str) -> Result<()> {
    // Open the key with read access to enumerate subkeys
    if let Ok(key) = hkcu.open_subkey_with_flags(path, KEY_READ | KEY_WRITE) {
        // Delete all subkeys first (recursive)
        for sub in key.enum_keys().collect::<Vec<_>>() {
            if let Ok(sub_name) = sub {
                let sub_path = format!(r"{}\{}", path, sub_name);
                let _ = delete_reg_tree(hkcu, &sub_path);
            }
        }
        // Now delete the key itself
        let _ = hkcu.delete_subkey(path);
    }
    Ok(())
}

/// GUID for FerroCopy shell extension (reserved for future COM DLL)
pub const FERROCOPY_SHELL_GUID: &str = "{F3C8B5A1-2D4E-4A6F-8B7C-9D0E1F2A3B4C}";

/// Context menu entry definitions: (registry_key_name, display_label, action_subcommand)
const MENU_ENTRIES: &[(&str, &str, &str)] = &[
    ("FerroCopy_Copy", "FerroCopyにコピー(&F)", "shell-copy"),
    ("FerroCopy_Move", "FerroCopyに移動(&M)", "shell-move"),
];

/// Registry scope paths under HKCU for context menu targets
const SCOPES: &[&str] = &[
    r"Software\Classes\*\shell",                    // 全ファイル
    r"Software\Classes\Directory\shell",             // フォルダ
    r"Software\Classes\AllFilesystemObjects\shell",  // ファイル＋フォルダ
];

/// Registry scope for folder background (paste target)
const BG_SCOPE: &str = r"Software\Classes\Directory\Background\shell";

/// FerroCopy exe path (resolved at call time)
fn ferrocopy_exe() -> Result<PathBuf> {
    std::env::current_exe().context("Failed to get current executable path")
}

/// Format a Windows command string for the context menu registry entry.
/// `%V` is the Explorer variable for the current directory (background),
/// `%1` for the selected item.
fn command_line(exe: &std::path::Path, action: &str) -> String {
    format!(
        "\"{}\" {} \"%1\"",
        exe.display(),
        action
    )
}

fn bg_command_line(exe: &std::path::Path, action: &str) -> String {
    format!(
        "\"{}\" {} \"%V\"",
        exe.display(),
        action
    )
}

/* ── Install ─────────────────────────────────────────────────────────── */

/// Install ALL shell integration features
pub fn install() -> Result<()> {
    let exe = ferrocopy_exe()?;
    install_context_menu(&exe)?;
    install_background_menu(&exe)?;
    install_sendto(&exe)?;
    install_copy_handler_placeholder(&exe)?;

    // Phase 3: Win11 modern context menu (IExplorerCommand)
    match install_explorer_command(&exe) {
        Ok(()) => {}
        Err(e) => {
            // Don't fail the whole install if DLL is missing — print warning
            eprintln!("⚠️  Win11 modern menu skipped: {}", e);
        }
    }

    notify_shell_change()?;
    println!("✅ FerroCopy shell integration installed.");
    println!("   • Right-click: 'FerroCopyにコピー' / 'FerroCopyに移動'");
    println!("   • Folder bg:   'FerroCopyにペースト'");
    println!("   • Send To:     'FerroCopy'");
    println!("   • Win11 menu:  IExplorerCommand (if DLL present)");
    Ok(())
}

/// Uninstall ALL shell integration features
pub fn uninstall() -> Result<()> {
    uninstall_context_menu()?;
    uninstall_background_menu()?;
    uninstall_sendto()?;
    uninstall_copy_handler_placeholder()?;
    uninstall_explorer_command()?;
    notify_shell_change()?;
    println!("✅ FerroCopy shell integration removed.");
    Ok(())
}

/* ── Context Menu Registration ──────────────────────────────────────── */

fn install_context_menu(exe: &std::path::Path) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    for scope in SCOPES {
        for (key_name, label, action) in MENU_ENTRIES {
            let parent_path = format!(r"{}\{}", scope, key_name);
            let (parent_key, _) = hkcu
                .create_subkey(&parent_path)
                .with_context(|| format!("Failed to create registry key: {}", parent_path))?;

            // (Default) = display label
            parent_key.set_value("", &label.to_string())?;
            // Icon = path to exe
            parent_key.set_value("Icon", &format!("\"{}\"", exe.display()))?;
            // For multiple selections, show for each
            parent_key.set_value("MultiSelectModel", &"Player".to_string())?;

            // command subkey
            let cmd_path = format!(r"{}\command", parent_path);
            let (cmd_key, _) = hkcu
                .create_subkey(&cmd_path)
                .with_context(|| format!("Failed to create command key: {}", cmd_path))?;
            cmd_key.set_value("", &command_line(exe, action))?;
        }
    }

    Ok(())
}

fn install_background_menu(exe: &std::path::Path) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    // フォルダ背景右クリックに「FerroCopyにペースト」を追加
    let bg_path = format!(r"{}\FerroCopy_Paste", BG_SCOPE);
    let (bg_key, _) = hkcu
        .create_subkey(&bg_path)
        .context("Failed to create background menu key")?;
    bg_key.set_value("", &"FerroCopyにペースト".to_string())?;
    bg_key.set_value("Icon", &format!("\"{}\"", exe.display()))?;

    let bg_cmd_path = format!(r"{}\command", bg_path);
    let (bg_cmd_key, _) = hkcu
        .create_subkey(&bg_cmd_path)
        .context("Failed to create background command key")?;
    bg_cmd_key.set_value("", &bg_command_line(exe, "shell-paste"))?;

    Ok(())
}

fn install_sendto(exe: &std::path::Path) -> Result<()> {
    // Send To フォルダにショートカットを作成
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let sendto = get_sendto_path()?;
    let shortcut_path = sendto.join("FerroCopy.lnk");

    // Windows Script Host でショートカット作成
    let vbs_script = format!(
        r#"
Set WshShell = WScript.CreateObject("WScript.Shell")
Set Shortcut = WshShell.CreateShortcut("{}")
Shortcut.TargetPath = "{}"
Shortcut.Description = "Copy files with FerroCopy"
Shortcut.Save
"#,
        shortcut_path.display().to_string().replace("\\", "\\\\"),
        exe.display().to_string().replace("\\", "\\\\"),
    );

    let vbs_path = std::env::temp_dir().join("ferrocopy_sendto.vbs");
    std::fs::write(&vbs_path, vbs_script)
        .context("Failed to write temporary VBS script")?;

    let output = Command::new("cscript.exe")
        .args([
            "/nologo",
            &vbs_path.to_string_lossy(),
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
        .context("Failed to create Send To shortcut")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("cscript.exe failed: {}", stderr);
    }

    let _ = std::fs::remove_file(&vbs_path);
    println!("   • Send To:     {}", shortcut_path.display());
    Ok(())
}

/// Get the SendTo folder path
fn get_sendto_path() -> Result<PathBuf> {
    // Use SHGetFolderPath via CSIDL_SENDTO
    let sendto_key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders",
            KEY_READ,
        )
        .or_else(|_| {
            RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
                r"Software\Microsoft\Windows\CurrentVersion\Explorer\Shell Folders",
                KEY_READ,
            )
        })
        .context("Failed to open Shell Folders registry")?;

    let path: String = sendto_key
        .get_value("{9E3995AB-1F9C-4F13-B827-48B24B6C7174}")
        .or_else(|_| sendto_key.get_value("SendTo"))
        .unwrap_or_else(|_| {
            // Fallback to default path
            let home = std::env::var("USERPROFILE")
                .unwrap_or_else(|_| "C:\\Users\\Default".to_string());
            format!("{}\\AppData\\Roaming\\Microsoft\\Windows\\SendTo", home)
        });

    // Expand environment variables (%USERPROFILE% etc.)
    let path_clone = path.clone();
    let expanded = shellexpand::full(&path_clone)
        .unwrap_or(std::borrow::Cow::Owned(path));
    Ok(PathBuf::from(expanded.as_ref()))
}

/// Register CopyHook handler placeholder (for future COM DLL).
/// Without the DLL, these keys won't activate, but they reserve the
/// GUID for when the COM server is built.
fn install_copy_handler_placeholder(_exe: &std::path::Path) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    // CopyHookHandler for directories
    let copy_hook_path = r"Software\Classes\Directory\shellex\CopyHookHandlers\FerroCopy";
    let (ch_key, _) = hkcu
        .create_subkey(copy_hook_path)
        .context("Failed to create CopyHook handler key")?;
    ch_key.set_value("", &FERROCOPY_SHELL_GUID.to_string())?;

    // DragDropHandler for directories
    let dd_path = r"Software\Classes\Directory\shellex\DragDropHandlers\FerroCopy";
    let (dd_key, _) = hkcu
        .create_subkey(dd_path)
        .context("Failed to create DragDrop handler key")?;
    dd_key.set_value("", &FERROCOPY_SHELL_GUID.to_string())?;

    Ok(())
}

/// Register IExplorerCommand for Win11 modern context menu (Phase 3).
///
/// Registers the COM DLL under HKCU\Software\Classes\CLSID and
/// shellex\ContextMenuHandlers so that Win11 shows FerroCopy in the
/// condensed modern context menu (not just "Show more options").
///
/// Requires ferrocopy-hook.dll to be built and registered via regsvr32
/// or DllRegisterServer first, OR the CLSID entries below allow the
/// shell to load the DLL directly from the ferrocopy.exe directory.
fn install_explorer_command(exe: &std::path::Path) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let exe_dir = exe.parent().context("Failed to get exe directory")?;
    let dll_path = exe_dir.join("ferrocopy-hook.dll");

    // If the DLL doesn't exist next to the exe, fall back to the in-tree
    // build output path so --install-context works from cargo run.
    let dll_path = if dll_path.exists() {
        dll_path
    } else {
        // Try target/release/ferrocopy-hook.dll relative to manifest dir
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest.join("target").join("release").join("ferrocopy-hook.dll")
    };

    if !dll_path.exists() {
        anyhow::bail!(
            "ferrocopy-hook.dll not found at '{}'. Build it first with: cargo build -p ferrocopy-hook --release",
            dll_path.display()
        );
    }

    let dll_abs = std::fs::canonicalize(&dll_path)
        .context("Failed to resolve DLL path")?;
    let dll_str = dll_abs.display().to_string();

    let clsid = FERROCOPY_SHELL_GUID;

    // CLSID entry under HKCU (no admin required)
    let clsid_path = format!(r"Software\Classes\CLSID\{}", clsid);
    let (clsid_key, _) = hkcu
        .create_subkey(&clsid_path)
        .context("Failed to create CLSID key")?;
    clsid_key.set_value("", &"FerroCopy Shell Extension".to_string())?;

    // InprocServer32 (DLL path)
    let inproc_path = format!(r"{}\InprocServer32", clsid_path);
    let (inproc_key, _) = hkcu
        .create_subkey(&inproc_path)
        .context("Failed to create InprocServer32 key")?;
    inproc_key.set_value("", &dll_str)?;
    inproc_key.set_value("ThreadingModel", &"Apartment".to_string())?;

    // IExplorerCommand registration (Win11 modern menu)
    let explorer_cmd_path = format!(r"Software\Classes\CLSID\{}\ExplorerCommandHandler", clsid);
    let (exp_key, _) = hkcu
        .create_subkey(&explorer_cmd_path)
        .context("Failed to create ExplorerCommandHandler key")?;
    exp_key.set_value("", &clsid.to_string())?;

    // shellex\ContextMenuHandlers for both IContextMenu and IExplorerCommand
    for scope in SCOPES {
        let handler_path = format!(r"{}\shellex\ContextMenuHandlers\FerroCopy", scope);
        let (handler_key, _) = hkcu
            .create_subkey(&handler_path)
            .context("Failed to create ContextMenuHandlers key")?;
        handler_key.set_value("", &clsid.to_string())?;
    }

    println!("   • Win11 modern menu: IExplorerCommand registered (CLSID {})", clsid);
    println!("   • DLL: {}", dll_str);
    Ok(())
}

/// Unregister IExplorerCommand (Phase 3)
fn uninstall_explorer_command() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let clsid = FERROCOPY_SHELL_GUID;

    // Remove CLSID tree
    let clsid_path = format!(r"Software\Classes\CLSID\{}", clsid);
    let _ = delete_reg_tree(&hkcu, &clsid_path);

    // Remove shellex\ContextMenuHandlers
    for scope in SCOPES {
        let handler_path = format!(r"{}\shellex\ContextMenuHandlers\FerroCopy", scope);
        let _ = delete_reg_tree(&hkcu, &handler_path);
    }

    Ok(())
}

/* ── Uninstall ───────────────────────────────────────────────────────── */

fn uninstall_context_menu() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    for scope in SCOPES {
        for (key_name, _, _) in MENU_ENTRIES {
            let path = format!(r"{}\{}", scope, key_name);
            let _ = delete_reg_tree(&hkcu, &path);
        }
    }

    Ok(())
}

fn uninstall_background_menu() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = format!(r"{}\FerroCopy_Paste", BG_SCOPE);
    let _ = delete_reg_tree(&hkcu, &path);
    Ok(())
}

fn uninstall_sendto() -> Result<()> {
    if let Ok(sendto) = get_sendto_path() {
        let shortcut = sendto.join("FerroCopy.lnk");
        let _ = std::fs::remove_file(&shortcut);
    }
    Ok(())
}

fn uninstall_copy_handler_placeholder() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let _ = delete_reg_tree(&hkcu, r"Software\Classes\Directory\shellex\CopyHookHandlers\FerroCopy");
    let _ = delete_reg_tree(&hkcu, r"Software\Classes\Directory\shellex\DragDropHandlers\FerroCopy");

    Ok(())
}

/* ── Shell notification ─────────────────────────────────────────────── */

/// Notify Windows Explorer that shell settings have changed
fn notify_shell_change() -> Result<()> {
    // Use HWND_BROADCAST + WM_SETTINGCHANGE to refresh Explorer
    // On failure, tell user to restart Explorer
    unsafe {
        use windows::Win32::Foundation::{HWND, WPARAM, LPARAM};
        let _ = windows::Win32::UI::WindowsAndMessaging::SendNotifyMessageW(
            HWND(usize::MAX as *mut std::ffi::c_void), // HWND_BROADCAST
            windows::Win32::UI::WindowsAndMessaging::WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(0), // Environment changes - let Explorer refresh all
        );
    }
    Ok(())
}

/* ── Shell action handlers (called by main.rs) ──────────────────────── */

/// Handle `shell-copy` invocation from context menu.
/// Opens the GUI with pre-populated source files.
pub fn handle_shell_copy(paths: Vec<String>) -> Result<()> {
    if paths.is_empty() {
        anyhow::bail!("No files specified for shell copy");
    }

    // Filter valid paths
    let valid: Vec<String> = paths
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect();

    if valid.is_empty() {
        anyhow::bail!("No valid source files");
    }

    // Open GUI with the files pre-loaded
    gui::run_gui_with_sources(valid);

    Ok(())
}

/// Handle `shell-move` invocation from context menu.
pub fn handle_shell_move(paths: Vec<String>) -> Result<()> {
    // Same as shell-copy but sets move mode
    if paths.is_empty() {
        anyhow::bail!("No files specified for shell move");
    }

    let valid: Vec<String> = paths
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect();

    if valid.is_empty() {
        anyhow::bail!("No valid source files");
    }

    gui::run_gui_with_sources_move(valid);

    Ok(())
}

/// Handle `shell-paste` from folder background.
/// Opens GUI with the folder as destination.
pub fn handle_shell_paste(target_dir: String) -> Result<()> {
    let path = std::path::Path::new(&target_dir);
    if !path.is_dir() {
        anyhow::bail!("Invalid paste target: not a directory");
    }

    gui::run_gui_with_destination(target_dir);

    Ok(())
}