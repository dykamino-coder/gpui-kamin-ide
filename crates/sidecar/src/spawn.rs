//! Спавн одного host-процесса + чтение stdout до его смерти
//! (порт sidecar.rs::run_once; embed-runtime режим добавится в фазе packaging).

use std::io::{BufRead as _, BufReader};
#[cfg(windows)]
use std::os::windows::process::CommandExt as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::HostState;
use crate::job::assign_to_job;
use crate::ready::{HostEndpoint, parse_ready};

/// node.exe — console-binary: без флага из GUI-шелла выскочит консоль.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub enum HostMode {
    /// Dev: репо kamin-ide, `node --import tsx src/kamin-host/kamin-host.ts`.
    DevRepo { repo_root: PathBuf },
    /// Packaged: распакованный runtime (kaminhost.exe + kamin-host.mjs) — фаза packaging.
    Runtime { runtime_dir: PathBuf },
}

pub struct HostConfig {
    pub mode: HostMode,
    /// Версия native shell — единый build id для incident-записей Node host и
    /// extension host. Передаётся env, пользовательские данные сюда не входят.
    pub app_version: &'static str,
    /// НАШИ данные (не трогаем прод studio.dykamino.kaminide, пока живём рядом).
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    /// «Open with KaminIDE»-цепочка (плюс dev-удобство).
    pub open_folder: Option<PathBuf>,
}

/// Один запуск: спавн, ready-парсинг, ожидание смерти.
/// true = хост хотя бы раз стал ready (сброс бюджета рестартов).
pub fn run_once(
    config: &HostConfig,
    state: &HostState,
    on_endpoint: &(dyn Fn(HostEndpoint) + Send + Sync),
) -> bool {
    let mut command = match &config.mode {
        HostMode::DevRepo { repo_root } => {
            // СОБРАННЫЙ бандл, если он есть: `--import tsx` транспилирует весь
            // граф хоста на КАЖДОМ старте (и повторно в форкнутом exthost-
            // ребёнке) — замер показал ~12 с, за которые стоят ВСЕ RPC, отсюда
            // «панелей нет минуту» и «0 active». С бандлом это единицы секунд.
            // Собирается `npm run build:host:tauri` (0.9 с, 445 КБ).
            let bundle = repo_root.join("dist-host").join("kamin-host.mjs");
            let mut cmd = Command::new("node");
            cmd.current_dir(repo_root);
            if bundle.is_file() {
                // KAMIN_CPU_PROF=1 — снять профиль родителя хоста: блоки цикла
                // по 400-520 мс остались после того, как транспорт (WS-отправка
                // и приём кадров от ребёнка) оказался чист.
                if std::env::var("KAMIN_CPU_PROF").as_deref() == Ok("1") {
                    cmd.args(["--cpu-prof", "--cpu-prof-dir"])
                        .arg(std::env::temp_dir().join("kamin-cpuprof"));
                }
                cmd.arg(bundle);
            } else {
                cmd.args(["--import", "tsx", "src/kamin-host/kamin-host.ts"]);
            }
            cmd.arg(format!(
                "--builtin-dir={}",
                repo_root.join("builtin-extensions").display()
            ));
            cmd
        }
        HostMode::Runtime { runtime_dir } => {
            // kaminhost.exe = переименованный node.exe (анти taskkill /IM node.exe)
            let host_exe = runtime_dir.join("kaminhost.exe");
            let node_exe = runtime_dir.join("node.exe");
            let bin = if host_exe.exists() {
                host_exe
            } else {
                node_exe
            };
            let mut cmd = Command::new(bin);
            cmd.current_dir(runtime_dir)
                .arg(runtime_dir.join("kamin-host.mjs"))
                .arg(format!(
                    "--builtin-dir={}",
                    runtime_dir.join("builtin-extensions").display()
                ));
            cmd
        }
    };

    command
        .env("KAMIN_HOST_TRANSPORT", "stdio")
        .env("KAMIN_APP_VERSION", config.app_version)
        // Индекс-walker гигантского воркспейса сатурирует дефолтные 4 потока
        // libuv → fs-RPC (listDir/exists) голодают МИНУТАМИ (Q5-смежное,
        // диагностировано 2026-07-24). Расширение пула — митигация.
        .env("UV_THREADPOOL_SIZE", "16")
        .arg(format!("--data-dir={}", config.data_dir.display()))
        .arg(format!("--cache-dir={}", config.cache_dir.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(folder) = &config.open_folder {
        command.arg(format!("--open-folder={}", folder.display()));
    }
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            eprintln!("failed to spawn kamin-host (is `node` on PATH?): {err}");
            return false;
        }
    };

    assign_to_job(&child);

    // stderr хоста → наш лог (у него нет консоли — CREATE_NO_WINDOW)
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("[kamin-host] {line}");
            }
        });
    }

    let mut became_ready = false;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Some(endpoint) = parse_ready(&line) {
                became_ready = true;
                eprintln!("kamin-host ready on port {}", endpoint.port);
                state.set(endpoint.clone());
                on_endpoint(endpoint);
            }
        }
    }

    match child.wait() {
        Ok(status) => eprintln!("kamin-host exited: {status}"),
        Err(err) => eprintln!("kamin-host wait failed: {err}"),
    }
    became_ready
}
