//! kamin-probe — debug-RPC для нативного окна (plan/96 §0): замена CDP.
//! Протокол: TCP loopback, JSON-lines (одна строка = один запрос/ответ).
//! Запрос:  {"cmd":"ping"|"tree"|"metric"|"click"|"screenshot", "token":"...", ...}
//! Ответ:   {"ok":true, ...} | {"ok":false,"err":"..."}
//! В release-сборке крейт не линкуется (feature `probe` в shell).

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use serde_json::{Value, json};

/// Обработчик команд. Реализация в shell маршрутизирует tree/metric/click/
/// screenshot в GPUI foreground-поток; здесь — только транспорт.
pub trait ProbeHandler: Send + Sync + 'static {
    fn handle(&self, cmd: &str, req: &Value) -> Value;
}

pub struct ProbeServer {
    pub port: u16,
}

impl ProbeServer {
    /// Стартует listener-поток на 127.0.0.1:`port` (0 = эфемерный).
    /// `token` — обязателен в каждом запросе, если задан.
    pub fn start(
        port: u16,
        token: Option<String>,
        handler: Arc<dyn ProbeHandler>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let actual_port = listener.local_addr()?.port();
        thread::Builder::new()
            .name("kamin-probe".into())
            .spawn(move || {
                // Паника в обработчике КЛИЕНТА не должна убивать accept-цикл:
                // ревьюеры теряли стенд посреди замеров — probe переставал
                // слушать при живом приложении (отчёты ц.7 и ц.10).
                for stream in listener.incoming().flatten() {
                    let handler = handler.clone();
                    let token = token.clone();
                    thread::spawn(move || {
                        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            serve_client(stream, token, handler);
                        }));
                        if r.is_err() {
                            eprintln!(
                                "kamin-probe: обработчик клиента упал, сервер продолжает слушать"
                            );
                        }
                    });
                }
            })?;
        Ok(Self { port: actual_port })
    }
}

fn serve_client(stream: TcpStream, token: Option<String>, handler: Arc<dyn ProbeHandler>) {
    let reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut writer = stream;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let resp = respond(&line, token.as_deref(), handler.as_ref());
        let mut out = resp.to_string();
        out.push('\n');
        if writer.write_all(out.as_bytes()).is_err() {
            break;
        }
    }
}

fn respond(line: &str, token: Option<&str>, handler: &dyn ProbeHandler) -> Value {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return json!({"ok": false, "err": format!("bad json: {e}")}),
    };
    if let Some(expected) = token
        && req.get("token").and_then(Value::as_str) != Some(expected)
    {
        return json!({"ok": false, "err": "bad token"});
    }
    let Some(cmd) = req.get("cmd").and_then(Value::as_str) else {
        return json!({"ok": false, "err": "missing cmd"});
    };
    handler.handle(cmd, &req)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;
    impl ProbeHandler for Echo {
        fn handle(&self, cmd: &str, _req: &Value) -> Value {
            json!({"ok": true, "cmd": cmd})
        }
    }

    #[test]
    fn ping_roundtrip_and_token_gate() {
        let srv = ProbeServer::start(0, Some("s3cret".into()), Arc::new(Echo)).unwrap();
        let mut c = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
        c.write_all(b"{\"cmd\":\"ping\",\"token\":\"s3cret\"}\n{\"cmd\":\"ping\"}\n")
            .unwrap();
        let mut r = BufReader::new(c);
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        assert!(line.contains("\"ok\":true"), "{line}");
        line.clear();
        r.read_line(&mut line).unwrap();
        assert!(line.contains("bad token"), "{line}");
    }
}
