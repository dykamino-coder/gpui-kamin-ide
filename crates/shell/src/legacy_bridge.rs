//! Порт `src-tauri/src/bridge_uninstall.rs` из kamin-ide 1:1 (Tauri-обвязка
//! снята): обнаружение и удаление СТАРОГО Electron-Bridge. Используется
//! карточкой «Legacy Electron Bridge detected» на экране Settings.

//! Legacy Electron Bridge cleanup — detect + remove the old
//! `open-claude-bridge-electron` desktop client that KaminIDE replaces.
//!
//! Upgrading users had the Electron client installed (Squirrel, HKCU/no-admin).
//! KaminIDE already imports its sessions into the native project tree (kamin-host
//! `importBridgeSessions`); this module lets the user retire the old app from
//! inside KaminIDE: run its own uninstaller, drop its
//! `Directory\shell\OpenClaudeBridge` folder context-menu entries, and delete
//! its on-disk config. Every target is a FIXED, HKCU/%APPDATA%-scoped path we
//! verify exists before touching — no admin, no user-supplied paths, so
//! `remove_dir_all` can never be pointed at an arbitrary location.

/// The old app's folder context-menu keys — mirror of the Electron
/// `installer-helpers.ts` `unregisterContextMenu`.
#[cfg(windows)]
const BRIDGE_MENU_KEYS: [&str; 2] = [
    r"Software\Classes\Directory\shell\OpenClaudeBridge",
    r"Software\Classes\Directory\Background\shell\OpenClaudeBridge",
];

/// Squirrel per-user uninstall registry key.
#[cfg(windows)]
const UNINSTALL_KEY: &str =
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenClaudeBridge";

/// What traces of the old Electron Bridge exist on this machine. Any `true`
/// means the "remove old Bridge" button should show.
#[derive(Default)]
pub struct BridgeFootprint {
    pub found: bool,
    /// Installed via Squirrel (Update.exe present or an Uninstall entry).
    pub squirrel: bool,
    /// Explorer folder "Open with" context-menu entry present.
    pub context_menu: bool,
    /// On-disk config dir present.
    pub config: bool,
}

/// `%APPDATA%\open-claude-bridge-electron` — unambiguously the Bridge's config.
fn config_dir() -> Option<std::path::PathBuf> {
    std::env::var("APPDATA")
        .ok()
        .map(|a| std::path::Path::new(&a).join("open-claude-bridge-electron"))
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// True if a registry key exists (`reg query` succeeds).
#[cfg(windows)]
fn reg_key_exists(hk_key: &str) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    Command::new("reg")
        .args(["query", hk_key])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Read one string value from a registry key, or `None`.
#[cfg(windows)]
fn read_reg_value(hk_key: &str, name: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    let out = Command::new("reg")
        .args(["query", hk_key, "/v", name])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // A `/v <name>` query prints `    <name>    REG_SZ    <data>` (data may hold
    // spaces). Match either plain or expandable string types.
    for line in text.lines() {
        for ty in ["REG_EXPAND_SZ", "REG_SZ"] {
            if let Some(pos) = line.find(ty) {
                let val = line[pos + ty.len()..].trim();
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

/// Delete a registry key with `/f`; reports whether it actually existed.
#[cfg(windows)]
fn reg_delete(hk_key: &str) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    if !reg_key_exists(hk_key) {
        return false;
    }
    Command::new("reg")
        .args(["delete", hk_key, "/f"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// True if the app was installed via Squirrel.
#[cfg(windows)]
fn squirrel_present() -> bool {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let update = std::path::Path::new(&local)
            .join("OpenClaudeBridge")
            .join("Update.exe");
        if update.exists() {
            return true;
        }
    }
    reg_key_exists(UNINSTALL_KEY)
}

/// The command line that uninstalls the Squirrel install, if any. Prefers the
/// silent `QuietUninstallString`, then `UninstallString`, then a raw
/// `Update.exe --uninstall`.
#[cfg(windows)]
fn squirrel_uninstall_cmd() -> Option<String> {
    for name in ["QuietUninstallString", "UninstallString"] {
        if let Some(v) = read_reg_value(UNINSTALL_KEY, name) {
            return Some(v);
        }
    }
    let local = std::env::var("LOCALAPPDATA").ok()?;
    let update = std::path::Path::new(&local)
        .join("OpenClaudeBridge")
        .join("Update.exe");
    update
        .exists()
        .then(|| format!("\"{}\" --uninstall -s", update.display()))
}

/// Run a full command line through `cmd /C` (verbatim — the string keeps its
/// own quoting). Blocks until the uninstaller finishes so a follow-up re-detect
/// sees the removed install.
#[cfg(windows)]
fn run_uninstaller(cmd_line: &str) {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    let _ = Command::new("cmd")
        .raw_arg(format!("/C {cmd_line}"))
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

/// Report which traces of the old Electron Bridge exist. Drives the visibility
/// of the "remove old Bridge" button in Settings.
pub fn detect_electron_bridge() -> BridgeFootprint {
    #[cfg(windows)]
    {
        let squirrel = squirrel_present();
        let context_menu = BRIDGE_MENU_KEYS
            .iter()
            .any(|k| reg_key_exists(&format!("HKCU\\{k}")));
        let config = config_dir().map(|p| p.exists()).unwrap_or(false);
        BridgeFootprint {
            found: squirrel || context_menu || config,
            squirrel,
            context_menu,
            config,
        }
    }
    #[cfg(not(windows))]
    {
        BridgeFootprint::default()
    }
}

/// Remove the old Electron Bridge: run its uninstaller (if installed), drop its
/// folder context-menu entries, and delete its config. The caller (renderer)
/// re-imports sessions FIRST so nothing is lost when the config goes. Returns a
/// short human report of what was removed.
///
/// `async` so Tauri runs it on the async runtime, NOT the main thread — the
/// blocking `cmd /C <Squirrel uninstaller>` (which waits for the uninstaller to
/// fully exit) otherwise froze the whole UI ("Not Responding") for its duration.
pub fn uninstall_electron_bridge() -> Result<String, String> {
    #[cfg(windows)]
    {
        let mut done: Vec<&str> = Vec::new();

        // 1. Let the app uninstall itself (Squirrel) — removes the install +
        //    Start-menu shortcuts and runs its own `--squirrel-uninstall` hook.
        if let Some(cmd) = squirrel_uninstall_cmd() {
            run_uninstaller(&cmd);
            done.push("uninstalled app");
        }

        // 2. Drop the folder context-menu entries. Belt-and-suspenders: the
        //    Squirrel hook also does this, but a dev/portable copy never ran it.
        //    A plain loop (not `any`) so BOTH keys are always deleted.
        let mut menu_removed = false;
        for key in BRIDGE_MENU_KEYS {
            if reg_delete(&format!("HKCU\\{key}")) {
                menu_removed = true;
            }
        }
        if menu_removed {
            done.push("removed context-menu entry");
        }

        // 3. Delete the config dir (a fixed %APPDATA% path, checked to exist).
        if let Some(dir) = config_dir()
            && dir.exists()
            && std::fs::remove_dir_all(&dir).is_ok()
        {
            done.push("deleted config");
        }
        // The generic `%APPDATA%\Electron` dir is shared by any nameless dev
        // Electron app — only remove it when it holds the Bridge's own config.
        if let Ok(appdata) = std::env::var("APPDATA") {
            let generic = std::path::Path::new(&appdata).join("Electron");
            if generic.join("open-claude-bridge-config.json").exists()
                && std::fs::remove_dir_all(&generic).is_ok()
            {
                done.push("deleted legacy config");
            }
        }

        if done.is_empty() {
            Err("nothing to remove".into())
        } else {
            eprintln!("electron-bridge cleanup: {}", done.join(", "));
            Ok(done.join(", "))
        }
    }
    #[cfg(not(windows))]
    {
        Err("unsupported platform".into())
    }
}
