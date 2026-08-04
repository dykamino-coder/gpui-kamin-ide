//! Транспорт: blocking tungstenite, ОДИН io-поток на сокет.
//! Урок первой версии: два WebSocket-вью поверх try_clone() TcpStream —
//! unsound (reader авто-шлёт pong в тот же поток байт, что и writer:
//! интерливинг фреймов → тихая порча → зависшие ответы). Теперь один
//! WebSocket: цикл читает с read-timeout и между тиками сливает outbox.
//! Реконнект НЕ здесь: владелец (shell) пересоздаёт клиента, получив свежий
//! {port,token} от сайдкар-супервизора (hostEndpoint.onChanged-семантика).

use std::sync::Arc;
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
}

impl WsClient {
    /// Подключение к `ws://127.0.0.1:{port}/rpc?token={token}`.
    /// `on_disconnect` дёргается один раз при смерти сокета (после fail_all).
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
        std::thread::Builder::new()
            .name("kamin-ws-io".into())
            .spawn(move || {
                let mut ws = socket;
                // KAMIN_WS_TRACE=1 — трассировка кадров (диагностика транспорта)
                let trace = std::env::var("KAMIN_WS_TRACE").is_ok_and(|v| v == "1");
                'io: loop {
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

        Ok(Self { endpoint })
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
