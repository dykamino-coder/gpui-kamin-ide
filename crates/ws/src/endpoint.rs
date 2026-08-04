//! Корреляционное ядро RpcEndpoint (rpc.ts, 1:1 семантика), без сокета:
//! - request(): id-корреляция, ответ через канал
//! - хэндлеры host→клиент запросов (переживают реконнект — храним снаружи)
//! - fail_all() на разрыв ("kamin-host connection lost")
//!
//! Транспорт (tungstenite) — client.rs.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::frame::RpcFrame;

pub type RequestResult = Result<Value, String>;
/// Ответ на host→клиент запрос: сразу или отложенно (диалоги ждут юзера —
/// хендлер вернёт Later и позже вызовет `respond(id, …)`).
pub enum HostReply {
    Now(RequestResult),
    Later,
}
pub type HostRequestHandler = Arc<dyn Fn(u64, &str, &[Value]) -> HostReply + Send + Sync>;
pub type EventListener = Arc<dyn Fn(&str, &Value) + Send + Sync>;

#[derive(Default)]
struct Shared {
    next_id: u64,
    pending: HashMap<u64, Sender<RequestResult>>,
}

/// Ядро одного подключения. Живёт ровно столько, сколько живёт сокет
/// (реконнект = новый Endpoint, как новый RpcEndpoint в host-rpc.ts).
pub struct Endpoint {
    shared: Mutex<Shared>,
    outbox: Sender<RpcFrame>,
    pub on_event: EventListener,
    pub on_host_request: HostRequestHandler,
}

impl Endpoint {
    pub fn new(
        outbox: Sender<RpcFrame>,
        on_event: EventListener,
        on_host_request: HostRequestHandler,
    ) -> Self {
        Self {
            shared: Mutex::new(Shared::default()),
            outbox,
            on_event,
            on_host_request,
        }
    }

    /// Отправляет req и возвращает receiver результата.
    pub fn request(&self, method: &str, params: Vec<Value>) -> Receiver<RequestResult> {
        let (tx, rx) = channel();
        let id = {
            let mut s = self.shared.lock().unwrap();
            s.next_id += 1;
            let id = s.next_id;
            s.pending.insert(id, tx);
            id
        };
        let _ = self.outbox.send(RpcFrame::Req {
            id,
            method: method.into(),
            params,
        });
        rx
    }

    /// Обработка входящего кадра (вызывается reader-потоком транспорта).
    pub fn dispatch(&self, frame: RpcFrame) {
        match frame {
            RpcFrame::Res {
                id,
                ok,
                value,
                error,
            } => {
                let tx = self.shared.lock().unwrap().pending.remove(&id);
                if let Some(tx) = tx {
                    let result = if ok {
                        Ok(value.unwrap_or(Value::Null))
                    } else {
                        Err(error.unwrap_or_else(|| "unknown error".into()))
                    };
                    let _ = tx.send(result);
                }
            }
            RpcFrame::Evt { channel, payload } => (self.on_event)(&channel, &payload),
            RpcFrame::Req { id, method, params } => {
                // host→клиент запрос (shell.*) — ответить ОБЯЗАНЫ (plan/50 §2);
                // Later = диалог, ответ придёт через respond(id, …)
                match (self.on_host_request)(id, &method, &params) {
                    HostReply::Now(result) => self.respond(id, result),
                    HostReply::Later => {}
                }
            }
        }
    }

    /// Ответ на host→клиент запрос (немедленный или отложенный после Later).
    pub fn respond(&self, id: u64, result: RequestResult) {
        let res = match result {
            Ok(value) => RpcFrame::Res {
                id,
                ok: true,
                value: Some(value),
                error: None,
            },
            Err(e) => RpcFrame::Res {
                id,
                ok: false,
                value: None,
                error: Some(e),
            },
        };
        let _ = self.outbox.send(res);
    }

    /// Разрыв соединения: все висящие запросы получают ошибку (host-rpc.ts failAll).
    pub fn fail_all(&self, reason: &str) {
        let pending = std::mem::take(&mut self.shared.lock().unwrap().pending);
        for (_, tx) in pending {
            let _ = tx.send(Err(reason.to_string()));
        }
    }
}
