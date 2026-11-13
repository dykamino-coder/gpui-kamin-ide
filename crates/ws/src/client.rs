//! Транспорт: blocking tungstenite, ОДИН io-поток на сокет.
//! Урок первой версии: два WebSocket-вью поверх try_clone() TcpStream —
//! unsound (reader авто-шлёт pong в тот же поток байт, что и writer:
//! интерливинг фреймов → тихая порча → зависшие ответы). Теперь один
//! WebSocket: цикл читает с read-timeout и между тиками сливает outbox.
//! Реконнект НЕ здесь: владелец (shell) пересоздаёт клиента, получив свежий
//! {port,token} от сайдкар-супервизора (hostEndpoint.onChanged-семантика).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use serde_json::Value;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, connect};

use crate::endpoint::{Endpoint, EventListener, HostRequestHandler, RequestResult};
use crate::frame::RpcFrame;

pub const RETRY_DELAY_MS: u64 = 700; // host-rpc.ts:17
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Тик io-цикла: максимум столько исходящий кадр ждёт отправки.
const IO_TICK: Duration = Duration::from_millis(15);

pub struct WsClient {
    endpoint: Arc<Endpoint>,
    disconnect_requested: Arc<AtomicBool>,
}

impl WsClient {
    /// Подключение к `ws://127.0.0.1:{port}/rpc?token={token}`.
    /// `on_disconnect` дёргается один раз при смерти сокета (после fail_all).
    #[allow(
        clippy::print_stderr,
        clippy::disallowed_methods,
        reason = "preserve transport tracing until configuration and logging are passed explicitly"
    )]
    pub fn connect(
        port: u16,
        token: &str,
        on_event: EventListener,
        on_host_request: HostRequestHandler,
        on_disconnect: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        let url = format!("ws://127.0.0.1:{port}/rpc?token={token}");
        let (socket, _resp) = connect(&url).map_err(|e| format!("ws connect: {e}"))?;

        // read-timeout превращает блокирующий read в тик io-цикла
        match socket.get_ref() {
            MaybeTlsStream::Plain(stream) => stream
                .set_read_timeout(Some(IO_TICK))
                .map_err(|e| format!("set_read_timeout: {e}")),
            _ => Err("loopback ws must be plain tcp".to_string()),
        }?;

        let (writer_tx, writer_rx) = channel::<RpcFrame>();
        let endpoint = Arc::new(Endpoint::new(writer_tx, on_event, on_host_request));

        let ep = endpoint.clone();
        let disconnect_requested = Arc::new(AtomicBool::new(false));
        let disconnect = disconnect_requested.clone();
        std::thread::Builder::new()
            .name("kamin-ws-io".into())
            .spawn(move || {
                let mut ws = socket;
                // KAMIN_WS_TRACE=1 — трассировка кадров (диагностика транспорта)
                let trace = std::env::var("KAMIN_WS_TRACE").is_ok_and(|v| v == "1");
                'io: loop {
                    if disconnect.load(Ordering::Relaxed) {
                        let _ = ws.close(None);
                        break;
                    }
                    // 1) слить исходящие
                    loop {
                        match writer_rx.try_recv() {
                            Ok(frame) => {
                                let Ok(text) = serde_json::to_string(&frame) else {
                                    continue;
                                };
                                if trace {
                                    let dbg: String = text.chars().take(90).collect();
                                    eprintln!("[ws-io] send: {dbg}");
                                }
                                if let Err(e) = ws.send(Message::text(text)) {
                                    eprintln!("[ws-io] send failed: {e}");
                                    break 'io;
                                }
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => break,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => break 'io,
                        }
                    }
                    // 2) читать до таймаута тика (pong на ping tungstenite
                    //    шлёт сам — теперь безопасно: сокет один)
                    match ws.read() {
                        Ok(Message::Text(text)) => {
                            // Диагностика затора доставки: большие кадры и
                            // их частота видны без внешнего сниффера.
                            if trace || text.len() > 256 * 1024 {
                                eprintln!("[ws-io] recv {} KB", text.len() / 1024);
                            }
                            if let Ok(frame) = serde_json::from_str::<RpcFrame>(text.as_str()) {
                                ep.dispatch(frame);
                            }
                        }
                        Ok(Message::Close(_)) => break 'io,
                        Ok(_) => {} // ping/pong/binary
                        Err(tungstenite::Error::Io(e))
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) => {}
                        Err(_) => break 'io,
                    }
                }
                ep.fail_all("kamin-host connection lost");
                on_disconnect();
            })
            .map_err(|e| e.to_string())?;

        Ok(Self {
            endpoint,
            disconnect_requested,
        })
    }

    /// Close this connection on its IO thread; the owner's normal reconnect runs.
    pub fn disconnect(&self) {
        self.disconnect_requested.store(true, Ordering::Relaxed);
    }

    /// Блокирующий запрос (звать НЕ из UI-потока; из GPUI — background executor).
    pub fn request(&self, method: &str, params: Vec<Value>) -> RequestResult {
        let rx = self.endpoint.request(method, params);
        match rx.recv_timeout(REQUEST_TIMEOUT) {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) => Err(format!("request timeout: {method}")),
            Err(RecvTimeoutError::Disconnected) => Err("kamin-host connection lost".into()),
        }
    }

    /// Fire-and-forget (kamin:pty:write / resize — FORWARDED_SEND_CHANNELS).
    /// Хост всё равно ответит res — receiver дропаем, ответ снимет pending.
    pub fn send(&self, method: &str, params: Vec<Value>) {
        drop(self.endpoint.request(method, params));
    }

    /// Отложенный ответ на host→клиент запрос (после HostReply::Later).
    pub fn respond(&self, id: u64, result: RequestResult) {
        self.endpoint.respond(id, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HostReply;
    use std::net::TcpListener;

    #[test]
    fn disconnect_closes_socket_fails_pending_and_owner_reconnects_once() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (seen_tx, seen_rx) = channel();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            assert!(matches!(ws.read().unwrap(), Message::Text(_)));
            seen_tx.send(()).unwrap();
            assert!(matches!(ws.read().unwrap(), Message::Close(_)));
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut next = tungstenite::accept(stream).unwrap();
            let Message::Text(text) = next.read().unwrap() else {
                panic!("expected request")
            };
            let RpcFrame::Req { id, .. } = serde_json::from_str(text.as_str()).unwrap() else {
                panic!("expected request frame")
            };
            let reply = RpcFrame::Res {
                id,
                ok: true,
                value: Some(Value::Bool(true)),
                error: None,
            };
            next.send(Message::text(serde_json::to_string(&reply).unwrap()))
                .unwrap();
            assert!(matches!(next.read().unwrap(), Message::Close(_)));
        });
        let (closed_tx, closed_rx) = channel();
        let client = WsClient::connect(
            port,
            "fixture",
            Arc::new(|_, _| {}),
            Arc::new(|_, _, _| HostReply::Later),
            Arc::new(move || {
                let next = WsClient::connect(
                    port,
                    "fixture",
                    Arc::new(|_, _| {}),
                    Arc::new(|_, _, _| HostReply::Later),
                    Arc::new(|| {}),
                )
                .unwrap();
                closed_tx.send(next).unwrap();
            }),
        )
        .unwrap();
        let pending = client.endpoint.request("pending", vec![]);
        seen_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        client.disconnect();
        client.disconnect();
        assert_eq!(
            pending.recv_timeout(Duration::from_secs(5)).unwrap(),
            Err("kamin-host connection lost".into())
        );
        let next = closed_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            next.request("new-generation", vec![]),
            Ok(Value::Bool(true))
        );
        assert!(closed_rx.recv_timeout(Duration::from_millis(50)).is_err());
        next.disconnect();
        server.join().unwrap();
    }
}
