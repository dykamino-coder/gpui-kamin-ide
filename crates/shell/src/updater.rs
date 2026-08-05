//! Updater (порт UI-части Tauri-апдейтера оригинала): фоновая проверка
//! latest.json на канале обновлений → пилюля «Update vX ⭳» в статус-баре,
//! клик = скачать инсталлер (браузер/ShellExecute). Применение (подмена exe)
//! придёт с фазой дистрибуции; канал задаётся `KAMIN_GPUI_UPDATE_URL`
//! (дистрибуции GPUI-бинаря ещё нет — dev-сборка без URL канал не опрашивает).

use crate::host::events::CzEvent;
use serde_json::Value;
use smol::channel::Sender;

use crate::host_link::ShellEvent;

/// Найденное обновление: (версия, url инсталлера/страницы).
#[derive(Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
}

/// true если a новее b (сравнение по числовым компонентам "1.2.3").
pub fn version_newer(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.trim_start_matches('v')
            .split('.')
            .map(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    };
    let (va, vb) = (parse(a), parse(b));
    for i in 0..va.len().max(vb.len()) {
        let (x, y) = (
            va.get(i).copied().unwrap_or(0),
            vb.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    false
}

/// latest.json (Tauri-формат: {version, platforms:{windows-x86_64:{url}}}
/// или упрощённый {version, url}) → UpdateInfo если новее current.
pub fn parse_latest(body: &Value, current: &str) -> Option<UpdateInfo> {
    let version = body.get("version")?.as_str()?.to_string();
    if !version_newer(&version, current) {
        return None;
    }
    let url = body
        .get("platforms")
        .and_then(|p| p.get("windows-x86_64"))
        .and_then(|w| w.get("url"))
        .or_else(|| body.get("url"))
        .and_then(Value::as_str)?
        .to_string();
    Some(UpdateInfo { version, url })
}

/// Фоновая проверка канала (env `KAMIN_GPUI_UPDATE_URL`); тишина если канала
/// нет или сеть/формат не сложились (не dev-шум).
pub fn check_in_background(tx: Sender<ShellEvent>) {
    let Ok(url) = std::env::var("KAMIN_GPUI_UPDATE_URL") else {
        return;
    };
    std::thread::Builder::new()
        .name("kamin-updater".into())
        .spawn(move || {
            let Ok(mut resp) = ureq::get(&url).call() else {
                return;
            };
            let Ok(text) = resp.body_mut().read_to_string() else {
                return;
            };
            let Ok(body) = serde_json::from_str::<Value>(&text) else {
                return;
            };
            if let Some(info) = parse_latest(&body, env!("CARGO_PKG_VERSION")) {
                let _ = tx.try_send(ShellEvent::Cz(CzEvent::UpdateAvailable(
                    info.version,
                    info.url,
                )));
            }
        })
        .ok();
}

/// Журнал шагов установки в %TEMP%\kaminide-update.log: провалы самообновления
/// случаются ПОСЛЕ выхода приложения — тосты юзер не успевает прочитать.
fn ulog(msg: &str) {
    use std::io::Write as _;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::temp_dir().join("kaminide-update.log"))
    {
        let _ = writeln!(f, "[{}] {msg}", env!("CARGO_PKG_VERSION"));
    }
}

/// Каждые сколько байт слать событие прогресса (чаще — лишние кадры).
const TICK_BYTES: u64 = 256 * 1024;

/// Скачать инсталлятор во временный файл с отчётом прогресса.
fn download(url: &str, tx: &Sender<ShellEvent>) -> Result<std::path::PathBuf, String> {
    use std::io::{Read as _, Write as _};

    let resp = ureq::get(url).call().map_err(|e| e.to_string())?;
    let total: Option<u64> = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());

    // Имя строго с .exe: сервер до 6.3.89 отдавал url без имени файла
    // («…/artifact») — файл без расширения Windows не запускает, и установка
    // молча не происходила (инцидент «скачал до 100% и просто закрылся»).
    let name = url
        .rsplit('/')
        .next()
        .map(|n| n.split(['?', '#']).next().unwrap_or(n))
        .filter(|n| n.to_ascii_lowercase().ends_with(".exe"))
        .unwrap_or("KaminIDE-setup.exe");
    // Каталог ПРИЛОЖЕНИЯ, не %TEMP% и не отдельная папка: (а) AppLocker
    // блокирует запуск из Temp («blocked by group policy, 1260»), (б) задача
    // Планировщика (трамплин) из свежесозданного KaminIDE-updates падала с
    // 0x80070002 «файл не найден», а из каталога установки запускается
    // стабильно — приложение само оттуда работает.
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_else(std::env::temp_dir);
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;

    let mut body = resp.into_body().into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    let (mut done, mut last_tick) = (0u64, 0u64);
    // Полоса показывается сразу — «0 из неизвестного», как
    // `downloadProgress.value = { downloaded: 0, total: null }` оригинала
    let _ = tx.try_send(ShellEvent::Cz(CzEvent::UpdateProgress(0, total)));
    loop {
        let n = body.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if done - last_tick >= TICK_BYTES {
            last_tick = done;
            let _ = tx.try_send(ShellEvent::Cz(CzEvent::UpdateProgress(done, total)));
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    let _ = tx.try_send(ShellEvent::Cz(CzEvent::UpdateProgress(
        done,
        total.or(Some(done)),
    )));
    Ok(path)
}

/// Установка внутри приложения: скачать → запустить инсталлятор → выйти
/// (иначе он не перезапишет exe). Порт `updater_install` оригинала.
pub fn install(url: String, tx: Sender<ShellEvent>) {
    ulog(&format!("install start url={url}"));
    std::thread::spawn(move || match download(&url, &tx) {
        Ok(path) => {
            use std::os::windows::process::CommandExt as _;
            ulog(&format!("downloaded {}", path.display()));
            // Отвязанный запуск: инсталлер не должен умереть вместе с нами
            // (Job-объект родителя, консоль). BREAKAWAY может быть запрещён
            // Job'ом — тогда без него.
            const DETACH: u32 = 0x8 | 0x200; // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
            const BREAKAWAY: u32 = 0x0100_0000; // CREATE_BREAKAWAY_FROM_JOB
            let spawned = std::process::Command::new(&path)
                .arg("/update")
                .creation_flags(DETACH | BREAKAWAY)
                .spawn()
                .inspect(|_| ulog("spawned with BREAKAWAY"))
                .or_else(|e| {
                    ulog(&format!("BREAKAWAY spawn failed: {e} — fallback in-job"));
                    std::process::Command::new(&path)
                        .arg("/update")
                        .creation_flags(DETACH)
                        .spawn()
                });
            match spawned {
                Ok(mut child) => {
                    // Инцидент «скачал, закрыл приложение и всё»: мы выходили
                    // мгновенно, и смерть инсталлера на старте никто не видел.
                    // Даём ему прожить старт; мгновенный ненулевой выход =
                    // ошибка НА ЭКРАН, приложение остаётся жить. (Если он уже
                    // убил нас taskkill'ом из Install-секции — тем лучше.)
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    match child.try_wait() {
                        Ok(Some(status)) if !status.success() => {
                            ulog(&format!("installer exited early: {status}"));
                            let _ = tx.try_send(ShellEvent::Cz(CzEvent::UpdateInstallFailed(
                                format!("installer exited early: {status}"),
                            )));
                        }
                        _ => {
                            // НЕ process::exit(0): он пропускал CEF-шатдаун,
                            // грязный кэш ронял libcef на первом старте после
                            // апдейта (0x80000003 «у всех умерло, со второго
                            // запустилось»). Штатное закрытие окна доводит
                            // main() до web::shutdown(); инсталлер ждёт
                            // разблокировки файлов до 20с — успеваем.
                            ulog("installer alive/ok — graceful app quit");
                            let _ = tx.try_send(ShellEvent::Cz(CzEvent::GracefulQuit));
                        }
                    }
                }
                Err(e) => {
                    ulog(&format!("spawn failed: {e}"));
                    let _ =
                        tx.try_send(ShellEvent::Cz(CzEvent::UpdateInstallFailed(e.to_string())));
                }
            }
        }
        Err(e) => {
            ulog(&format!("download failed: {e}"));
            let _ = tx.try_send(ShellEvent::Cz(CzEvent::UpdateInstallFailed(e)));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{parse_latest, version_newer};

    #[test]
    fn newer_compare() {
        assert!(version_newer("0.2.5", "0.0.1"));
        assert!(version_newer("1.0.0", "0.9.9"));
        assert!(!version_newer("0.0.1", "0.0.1"));
        assert!(!version_newer("0.0.1", "0.2.0"));
        assert!(version_newer("v1.2", "1.1.9"));
    }

    #[test]
    fn parse_tauri_and_simple() {
        let tauri = serde_json::json!({
            "version": "9.9.9",
            "platforms": {"windows-x86_64": {"url": "https://x/y-setup.exe"}}
        });
        let got = parse_latest(&tauri, "0.0.1").unwrap();
        assert_eq!(got.version, "9.9.9");
        assert!(got.url.ends_with("setup.exe"));
        let simple = serde_json::json!({"version": "9.9.9", "url": "https://x/z"});
        assert!(parse_latest(&simple, "0.0.1").is_some());
        // Не новее → None
        assert!(parse_latest(&simple, "10.0.0").is_none());
    }
}
