//! Хост-сторона kamin-probe: маршрутизация команд (plan/96 §0).
//! tree/metric — из probe_registry (bounds пишутся на каждый prepaint);
//! screenshot — PrintWindow нашего окна (probe_shot);
//! click/key/... — синтетический ввод, следующая итерация.

pub(crate) use crate::probe::emit::emit;
use std::sync::Arc;

use kamin_probe::{ProbeHandler, ProbeServer};
use serde_json::{Value, json};

struct ShellProbe;

impl ProbeHandler for ShellProbe {
    fn handle(&self, cmd: &str, req: &Value) -> Value {
        crate::probe::cmds::inspect::handle_inspect(cmd, req)
            .or_else(|| crate::probe::cmds::input::handle_input(cmd, req))
            .or_else(|| crate::probe::cmds::app::handle_app(cmd, req))
            .unwrap_or_else(|| json!({"ok": false, "err": format!("unknown cmd: {cmd}")}))
    }
}

/// Запуск probe-сервера. Порт: `KAMIN_PROBE_PORT` (default 9333, 0 = эфемерный).
/// Токен: `KAMIN_PROBE_TOKEN` (опционален — loopback-only dev-инструмент).
pub fn start() {
    let port: u16 = std::env::var("KAMIN_PROBE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9333);
    let token = std::env::var("KAMIN_PROBE_TOKEN").ok();
    // Ретраи: при перезапуске порт пару секунд держит умирающий прошлый
    // процесс — одна неудачная попытка оставляла живой инстанс БЕЗ probe
    // (стенды падали «connection refused» при работающем приложении).
    std::thread::spawn(move || {
        for attempt in 1..=20 {
            match ProbeServer::start(port, token.clone(), Arc::new(ShellProbe)) {
                Ok(srv) => {
                    // Сервер живёт в своих потоках, хендл — только номер порта.
                    println!("kamin-probe: listening on 127.0.0.1:{}", srv.port);
                    return;
                }
                Err(e) => {
                    if attempt == 20 {
                        eprintln!("kamin-probe: failed to start on {port}: {e}");
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
        }
    });
}
