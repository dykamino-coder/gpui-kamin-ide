//! Шаги установки — порт NSIS-секции один в один, но без консольных окон:
//! все внешние процессы стартуют с CREATE_NO_WINDOW («3 консоли мигнули» —
//! жалоба юзера на ExecWait).

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Лог, совместимый с NSIS-эпохой: %TEMP%\kaminide-install.log, append.
pub fn klog(msg: &str) {
    use std::io::Write as _;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::temp_dir().join("kaminide-install.log"))
    {
        let _ = writeln!(f, "[{}] {msg}", env!("CARGO_PKG_VERSION"));
    }
}

fn quiet(cmd: &str, args: &[&str]) -> Result<i32> {
    let st = quiet_cmd(cmd, args).status().with_context(|| format!("spawn {cmd}"))?;
    Ok(st.code().unwrap_or(-1))
}

/// Команда с подавленным консольным окном — ЕДИНСТВЕННЫЙ способ порождать
/// процессы в инсталлере (юзер: «много консолей вспыхивало»). reg-query и
/// tasklist раньше звались напрямую и мигали окнами.
fn quiet_cmd(cmd: &str, args: &[&str]) -> Command {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt as _;
    let mut c = Command::new(cmd);
    c.args(args);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// InstallDir прошлой установки из реестра (юзер мог выбрать свой) — иначе
/// дефолт. Ровно семантика NSIS InstallDirRegKey.
pub fn resolve_install_dir() -> PathBuf {
    let from_reg = quiet_cmd("reg", &["query", r"HKCU\Software\KaminIDE-GPUI", "/v", "InstallDir"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            text.lines()
                .find(|l| l.contains("InstallDir"))
                .and_then(|l| l.split("REG_SZ").nth(1))
                .map(|p| PathBuf::from(p.trim()))
        });
    from_reg.unwrap_or_else(crate::default_install_dir)
}

pub fn in_job() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::JobObjects::IsProcessInJob;
        use windows::Win32::System::Threading::GetCurrentProcess;
        let mut in_job = windows::core::BOOL(0);
        IsProcessInJob(GetCurrentProcess(), None, &mut in_job).is_ok() && in_job.as_bool()
    }
    #[cfg(not(windows))]
    false
}

/// Пересоздать себя вне Job: CREATE_BREAKAWAY_FROM_JOB + DETACHED.
pub fn respawn_breakaway(args: &[String]) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const DETACH: u32 = 0x8 | 0x200;
        const BREAKAWAY: u32 = 0x0100_0000;
        let exe = std::env::current_exe()?;
        Command::new(exe)
            .args(args)
            .arg("/KAMINTRAMP")
            .creation_flags(DETACH | BREAKAWAY | CREATE_NO_WINDOW)
            .spawn()
            .context("breakaway spawn")?;
        Ok(())
    }
    #[cfg(not(windows))]
    bail!("windows only")
}

pub fn install(dir: &Path, version: &str) -> Result<()> {
    install_with_progress(dir, version, |_| {})
}

/// Установка с колбэком прогресса 0..100 (окно рисует полосу).
pub fn install_with_progress(dir: &Path, version: &str, progress: impl Fn(u8)) -> Result<()> {
    klog("install section start");
    progress(3);
    for image in ["kaminide-gpui.exe", "kaminhost.exe", "kaminide-web.exe"] {
        let _ = quiet("taskkill", &["/F", "/IM", image]);
    }
    klog("taskkill done");
    progress(8);

    // Реальное освобождение файлов (DLP держит хэндлы): проба переименования
    // главного exe, до 20×1с — порт wait_unlock.
    let main_exe = dir.join("kaminide-gpui.exe");
    if main_exe.exists() {
        let probe = dir.join("kaminide-gpui.exe.old");
        let mut ok = false;
        for attempt in 1..=20u32 {
            match std::fs::rename(&main_exe, &probe) {
                Ok(()) => {
                    std::fs::rename(&probe, &main_exe).ok();
                    klog(&format!("unlocked on attempt {attempt}"));
                    ok = true;
                    break;
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_secs(1)),
            }
        }
        if !ok {
            bail!("files still locked after 20s");
        }
    }
    progress(15);

    // Распаковка 553MB — самый долгий этап, а tar не даёт по-файловый прогресс.
    // Плавно ползём 15→86% по времени (≈оценка ~9с), пока идёт unpack; поток
    // гасим по флагу сразу как распаковка вернулась.
    let ticking = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let tick_flag = ticking.clone();
    let ticker = {
        // progress — не Send (impl Fn), поэтому тикаем прямо в ui-стор.
        std::thread::spawn(move || {
            let mut p = 15u8;
            while tick_flag.load(std::sync::atomic::Ordering::Relaxed) && p < 86 {
                std::thread::sleep(std::time::Duration::from_millis(180));
                p += 1;
                crate::ui::set_progress(p);
            }
        })
    };
    let unpacked = crate::payload::unpack_to(dir).context("payload unpack");
    ticking.store(false, std::sync::atomic::Ordering::Relaxed);
    let _ = ticker.join();
    unpacked?;
    klog("files_ok");
    progress(88);

    // Старые скачанные инсталлеры/трамплин-копии в каталоге приложения:
    // текущий запущенный залочен — молча пропускается, приберёт следующий цикл.
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if (name.starts_with("KaminIDE_") && name.ends_with("-setup.exe"))
                || name == "kaminide-selfupdate.exe"
            {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }

    std::fs::write(dir.join("version.txt"), version).context("version.txt")?;
    progress(92);
    write_registry(dir, version)?;
    write_shortcuts(dir)?;
    klog("registry+shortcuts done");
    progress(98);
    Ok(())
}

fn reg_add(key: &str, value: Option<&str>, data: &str) -> Result<()> {
    let mut args = vec!["add", key, "/f"];
    if let Some(v) = value {
        args.extend(["/v", v]);
    }
    args.extend(["/d", data]);
    let code = quiet("reg", &args)?;
    if code != 0 {
        bail!("reg add {key} → {code}");
    }
    Ok(())
}

fn write_registry(dir: &Path, version: &str) -> Result<()> {
    let exe = dir.join("kaminide-gpui.exe").display().to_string();
    let d = dir.display().to_string();
    reg_add(r"HKCU\Software\KaminIDE-GPUI", Some("InstallDir"), &d)?;
    // Контекст-меню папок: пишет ИНСТАЛЛЕР (self-heal «последний выигрывает»
    // оставлял пункт на старом exe). Сброс _Fingerprint → self-heal перерегистрируется.
    for base in [
        r"HKCU\Software\Classes\Directory\shell\OpenWithKaminIDE",
        r"HKCU\Software\Classes\Directory\Background\shell\OpenWithKaminIDE",
    ] {
        reg_add(base, None, "Open with KaminIDE")?;
        reg_add(base, Some("Icon"), &exe)?;
        reg_add(&format!(r"{base}\command"), None, &format!("\"{exe}\" \"%V\""))?;
    }
    let _ = quiet("reg", &["delete", r"HKCU\Software\Classes\Directory\shell\OpenWithKaminIDE", "/v", "_Fingerprint", "/f"]);
    let uk = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\KaminIDE-GPUI";
    reg_add(uk, Some("DisplayName"), "KaminIDE")?;
    reg_add(uk, Some("DisplayVersion"), version)?;
    reg_add(uk, Some("Publisher"), "dykamino.studio")?;
    reg_add(uk, Some("DisplayIcon"), &exe)?;
    reg_add(uk, Some("UninstallString"), &format!("\"{}\" /uninstall", dir.join("kaminide-setup.exe").display()))?;
    Ok(())
}

fn write_shortcuts(dir: &Path) -> Result<()> {
    let exe = dir.join("kaminide-gpui.exe").display().to_string();
    // WScript.Shell через PowerShell (скрыто): IShellLink на windows-rs — сотня
    // строк COM ради двух ярлыков.
    let script = format!(
        "$w = New-Object -ComObject WScript.Shell; \
         foreach ($p in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))) {{ \
           $s = $w.CreateShortcut((Join-Path $p 'KaminIDE.lnk')); $s.TargetPath = '{exe}'; $s.Save() }}"
    );
    let code = quiet("powershell", &["-NoProfile", "-Command", &script])?;
    if code != 0 {
        klog(&format!("shortcuts exit {code} (non-fatal)"));
    }
    Ok(())
}

/// Перезапуск приложения: сами (мы вне Job после трамплина) + проверка
/// живости с ретраями («у всех умерло, со второго раза запустилось»).
pub fn relaunch_app(dir: &Path) {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt as _;
    let exe = dir.join("kaminide-gpui.exe");
    for attempt in 1..=3u32 {
        let mut c = Command::new(&exe);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            c.creation_flags(0x8 | 0x200); // DETACHED | NEW_PROCESS_GROUP
        }
        let ok = c.spawn().is_ok();
        std::thread::sleep(std::time::Duration::from_secs(3));
        let alive = quiet_cmd("tasklist", &["/FI", "IMAGENAME eq kaminide-gpui.exe", "/NH"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("kaminide-gpui.exe"))
            .unwrap_or(false);
        klog(&format!("relaunch attempt {attempt} spawn={ok} alive={alive}"));
        if alive {
            return;
        }
    }
    klog("relaunch failed ×3");
}

pub fn uninstall() -> Result<()> {
    let dir = resolve_install_dir();
    for image in ["kaminide-gpui.exe", "kaminhost.exe", "kaminide-web.exe"] {
        let _ = quiet("taskkill", &["/F", "/IM", image]);
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    // Себя удалить нельзя (запущены) — чистим содержимое, свой exe пропускаем.
    let me = std::env::current_exe().ok();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if me.as_ref().is_some_and(|m| *m == e.path()) {
                continue;
            }
            let p = e.path();
            let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
        }
    }
    for key in [
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\KaminIDE-GPUI",
        r"HKCU\Software\Classes\Directory\shell\OpenWithKaminIDE",
        r"HKCU\Software\Classes\Directory\Background\shell\OpenWithKaminIDE",
        r"HKCU\Software\KaminIDE-GPUI",
    ] {
        let _ = quiet("reg", &["delete", key, "/f"]);
    }
    let script = "foreach ($p in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))) { Remove-Item -ErrorAction SilentlyContinue (Join-Path $p 'KaminIDE.lnk') }";
    let _ = quiet("powershell", &["-NoProfile", "-Command", script]);
    klog("uninstall done");
    Ok(())
}
