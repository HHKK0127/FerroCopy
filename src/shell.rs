//! FerroCopy Windows Shell Integration
//!
//! Provides:
//!   1. Right-click context menu "Copy with FerroCopy" / "Move with FerroCopy"
//!   2. Send To entry
//!   3. Explorer copy handler registration (placeholder for COM plugin)
//!
//! All registrations target HKEY_CURRENT_USER (no admin required).

use anyhow::{Context, Result};
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

/// GUID for FerroCopy shell extension (reserved for future COM DLL)
pub const FERROCOPY_SHELL_GUID: &str = "{F3C8B5A1-2D4E-4A6F-8B7C-9D0E1F2A3B4C}";

/// Context menu entry definitions: (registry_key_name, display_label, action_subcommand)
const MENU_ENTRIES: &[(&str, &str, &str)] = &[
    ("FerroCopy_Copy", "FerroCopyにコピー(&F)", "shell-copy"),
    ("FerroCopy_Move", "FerroCopyに移動(&M)", "shell-move"),
];

/// Registry scope paths under HKCU for context menu targets
const SCOPES: &[&str] = &[
    r"Software\Classes\*\shell",
    r"Software\Classes\Directory\shell",
    r"Software\Classes\AllFilesystemObjects\shell",
];

/// Folder background scope (paste target)
const BG_SCOPE: &str = r"Software\Classes\Directory\Background\shell";

fn ferrocopy_exe() -> Result<PathBuf> {
    std::env::current_exe().context("Failed to get current executable path")
}

fn command_line(exe: &std::path::Path, action: &str) -> String {
    format!("\"{}\" --{} \"%1\"", exe.display(), action)
}

fn bg_command_line(exe: &std::path::Path) -> String {
    format!("\"{}\" --shell-paste \"%V\"", exe.display())
}

/* ── Install ─────────────────────────────────────────────────────────── */

pub fn install() -> Result<()> {
    let exe = ferrocopy_exe()?;
    install_context_menu(&exe)?;
    install_background_menu(&exe)?;
    install_sendto(&exe)?;
    install_copy_handler_placeholder(&exe)?;
    notify_shell_change()?;
    println!("✅ FerroCopy shell integration installed.");
    println!("   • Right-click: 'FerroCopyにコピー' / 'FerroCopyに移動'");
    println!("   • Folder bg:   'FerroCopyにペースト'");
    println!("   • Send To:     'FerroCopy'");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    uninstall_context_menu()?;
    uninstall_background_menu()?;
    uninstall_sendto()?;
    uninstall_copy_handler_placeholder()?;
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

            parent_key.set_value("", &label.to_string())?;
            parent_key.set_value("Icon", &format!("\"{}\"", exe.display()))?;
            parent_key.set_value("MultiSelectModel", &"Player".to_string())?;

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
    bg_cmd_key.set_value("", &bg_command_line(exe))?;

    Ok(())
}

fn install_sendto(exe: &std::path::Path) -> Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let sendto = get_sendto_path()?;
    let shortcut_path = sendto.join("FerroCopy.lnk");

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
        .args(["/nologo", &vbs_path.to_string_lossy()])
        .creation_flags(0x08000000)
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

fn get_sendto_path() -> Result<PathBuf> {
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
            let home = std::env::var("USERPROFILE")
                .unwrap_or_else(|_| "C:\\Users\\Default".to_string());
            format!("{}\\AppData\\Roaming\\Microsoft\\Windows\\SendTo", home)
        });

    let expanded = shellexpand::full(&path)
        .unwrap_or_else(|_| std::borrow::Cow::Owned(path.clone()));
    Ok(PathBuf::from(expanded.as_ref()))
}

fn install_copy_handler_placeholder(_exe: &std::path::Path) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let copy_hook_path = r"Software\Classes\Directory\shellex\CopyHookHandlers\FerroCopy";
    let (ch_key, _) = hkcu
        .create_subkey(copy_hook_path)
        .context("Failed to create CopyHook handler key")?;
    ch_key.set_value("", &FERROCOPY_SHELL_GUID.to_string())?;

    let dd_path = r"Software\Classes\Directory\shellex\DragDropHandlers\FerroCopy";
    let (dd_key, _) = hkcu
        .create_subkey(dd_path)
        .context("Failed to create DragDrop handler key")?;
    dd_key.set_value("", &FERROCOPY_SHELL_GUID.to_string())?;

    Ok(())
}

/* ── Uninstall ───────────────────────────────────────────────────────── */

fn uninstall_context_menu() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for scope in SCOPES {
        for (key_name, _, _) in MENU_ENTRIES {
            let path = format!(r"{}\{}", scope, key_name);
            // winreg 0.52: delete_subkey_all recursively removes a key and all subkeys
            let _ = hkcu.delete_subkey_all(&path);
        }
    }
    Ok(())
}

fn uninstall_background_menu() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = format!(r"{}\FerroCopy_Paste", BG_SCOPE);
    let _ = hkcu.delete_subkey_all(&path);
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
    let _ = hkcu.delete_subkey_all(r"Software\Classes\Directory\shellex\CopyHookHandlers\FerroCopy");
    let _ = hkcu.delete_subkey_all(r"Software\Classes\Directory\shellex\DragDropHandlers\FerroCopy");
    Ok(())
}

/* ── Shell notification ─────────────────────────────────────────────── */

fn notify_shell_change() -> Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::Foundation::*;

    unsafe {
        // HWND_BROADCAST = HWND(-1 as isize as *mut _)
        let hwnd_broadcast = HWND((-1_isize) as *mut _);
        let _ = SendNotifyMessageW(
            hwnd_broadcast,
            WM_SETTINGCHANGE,
            WPARAM::default(),
            LPARAM(("Environment".encode_utf16().collect::<Vec<_>>().as_ptr()) as isize),
        );
    }
    Ok(())
}

/* ── Shell action handlers ──────────────────────────────────────────── */

pub fn handle_shell_copy(paths: Vec<String>) -> Result<()> {
    let valid: Vec<String> = paths.into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect();
    if valid.is_empty() {
        anyhow::bail!("No valid source files");
    }
    crate::gui::run_gui_with_sources(valid);
    Ok(())
}

pub fn handle_shell_move(paths: Vec<String>) -> Result<()> {
    let valid: Vec<String> = paths.into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect();
    if valid.is_empty() {
        anyhow::bail!("No valid source files");
    }
        crate::gui::run_gui_with_sources_move(valid);
    Ok(())
}

pub fn handle_shell_paste(target_dir: String) -> Result<()> {
    let path = std::path::Path::new(&target_dir);
    if !path.is_dir() {
        anyhow::bail!("Invalid paste target: not a directory");
    }
        crate::gui::run_gui_with_destination(target_dir);
    Ok(())
}