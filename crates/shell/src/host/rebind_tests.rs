//! INC-2026-0115: замена kamin-host под живым чатом проходит настоящим путём
//! шелла — WS-клиент, `connect_ws`, разбор событий вью и стор страниц, — а
//! хосты подменены минимальными WS-серверами того же протокола.
//!
//! Один тест: слоты клиента, отметки resolve и стор страниц глобальные.

use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kamin_sidecar::{HostEndpoint, HostState};
use kamin_ws::RpcFrame;
use serde_json::{Value, json};
use tungstenite::Message;

use crate::host::events::TermEvent;
use crate::host_link::ShellEvent;
use crate::ui::chat_webview::{CHAT_VIEW_ID, html_rev, stage_html};

const CHAT_HTML: &str = "<p>INC-2026-0115 chat</p>";
const WAIT: Duration = Duration::from_secs(10);

/// Хост с протоколом kamin-host: на resolve чата шлёт его HTML, как
/// `Webviews.resolveView`, на всё остальное отвечает `null`.
struct FakeHost {
    endpoint: HostEndpoint,
    resolves: Receiver<String>,
    connections: Arc<AtomicUsize>,
    dead: Arc<AtomicBool>,
}

impl FakeHost {
    fn start(token: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (resolve_tx, resolves) = channel();
        let resolve_tx = Arc::new(Mutex::new(resolve_tx));
        let connections = Arc::new(AtomicUsize::new(0));
        let dead = Arc::new(AtomicBool::new(false));
        let (count, stop) = (connections.clone(), dead.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let Ok((stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                };
                count.fetch_add(1, Ordering::SeqCst);
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_millis(20)))
                    .unwrap();
                let (stop, resolve_tx) = (stop.clone(), resolve_tx.clone());
                std::thread::spawn(move || serve(stream, &stop, &resolve_tx));
            }
            // Процесс умер: listener закрыт, новые подключения отклоняются.
        });
        Self {
            endpoint: HostEndpoint {
                port,
                token: token.into(),
            },
            resolves,
            connections,
            dead,
        }
    }

    fn kill(&self) {
        self.dead.store(true, Ordering::SeqCst);
    }

    fn wait_resolve(&self, id: &str) -> bool {
        let deadline = Instant::now() + WAIT;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match self.resolves.recv_timeout(left) {
                Ok(seen) if seen == id => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
        false
    }
}

fn serve(
    stream: std::net::TcpStream,
    stop: &AtomicBool,
    resolves: &Mutex<std::sync::mpsc::Sender<String>>,
) {
    let Ok(mut ws) = tungstenite::accept(stream) else {
        return;
    };
    let send = |ws: &mut tungstenite::WebSocket<_>, frame: RpcFrame| {
        ws.send(Message::text(serde_json::to_string(&frame).unwrap()))
    };
    while !stop.load(Ordering::SeqCst) {
        let text = match ws.read() {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => return,
            Ok(_) => continue,
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(_) => return,
        };
        let Ok(RpcFrame::Req { id, method, params }) = serde_json::from_str(text.as_str()) else {
            continue;
        };
        if method == "kamin:webviewView:resolve" {
            let view = params.first().and_then(Value::as_str).unwrap_or_default();
            let _ = resolves.lock().unwrap().send(view.to_string());
            if view == CHAT_VIEW_ID {
                let html = json!({ "id": view, "html": CHAT_HTML });
                let event = RpcFrame::Evt {
                    channel: "kamin:webviewView:html".into(),
                    payload: html,
                };
                if send(&mut ws, event).is_err() {
                    return;
                }
            }
        }
        let reply = RpcFrame::Res {
            id,
            ok: true,
            value: Some(Value::Null),
            error: None,
        };
        if send(&mut ws, reply).is_err() {
            return;
        }
    }
    // Смерть процесса рвёт сокет без закрывающего рукопожатия.
    drop(ws);
}

/// Шаг UI-цикла: HTML вью попадает в стор страниц (`apply_webview` → кадр).
/// Возвращает версию страницы и то, получила ли модель сигнал нового
/// extension host среди событий до этого HTML.
fn stage_next_chat_html(rx: &smol::channel::Receiver<ShellEvent>) -> Option<(u64, bool)> {
    let deadline = Instant::now() + WAIT;
    let mut respawn = false;
    while Instant::now() < deadline {
        match rx.try_recv() {
            Ok(ShellEvent::Term(TermEvent::WebviewHtml(id, html))) if id == CHAT_VIEW_ID => {
                stage_html(&id, html);
                return html_rev(&id).map(|rev| (rev, respawn));
            }
            Ok(ShellEvent::HostEvent(channel, _)) if channel == "kamin:exthost:respawned" => {
                respawn = true;
            }
            Ok(_) => {}
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    None
}

#[test]
fn replaced_host_rebinds_live_chat_once() {
    let _serial = crate::host::view_resolution::serial_test();
    // Холодный старт приложения: соседние тесты могли оставить отметки.
    crate::host_link::view_resolution().lock().unwrap().reset();
    let (tx, rx) = smol::channel::unbounded();
    let state = HostState::default();

    let first = FakeHost::start("first-host");
    state.set(first.endpoint.clone());
    super::super::connect::connect_ws(first.endpoint.clone(), tx.clone(), state.clone());
    assert!(
        first.wait_resolve(CHAT_VIEW_ID),
        "первый хост не получил resolve чата"
    );
    let (first_rev, cold_respawn) =
        stage_next_chat_html(&rx).expect("первый HTML чата не дошёл до стора");
    assert!(!cold_respawn, "холодный старт не замена хоста");

    // Процесс kamin-host умер; сайдкар поднимает новый с новым endpoint.
    first.kill();
    std::thread::sleep(Duration::from_millis(300));
    let second = FakeHost::start("second-host");
    state.set(second.endpoint.clone());
    let _ = tx.try_send(ShellEvent::HostReady(second.endpoint.clone()));
    super::super::connect::connect_ws(second.endpoint.clone(), tx, state.clone());

    assert!(
        second.wait_resolve(CHAT_VIEW_ID),
        "новый хост не получил resolve чата: его расширение не привяжет живую страницу",
    );
    let (second_rev, respawn) =
        stage_next_chat_html(&rx).expect("HTML нового хоста не дошёл до стора");
    assert!(
        second_rev > first_rev,
        "страница прежнего расширения не перезагружена новым хостом",
    );
    assert!(respawn, "модель не узнала о новом extension host");

    // Цепочка повторов старого клиента не подключается к новому хосту.
    std::thread::sleep(Duration::from_millis(3 * kamin_ws::RETRY_DELAY_MS));
    assert_eq!(
        second.connections.load(Ordering::SeqCst),
        1,
        "к новому хосту два WS-клиента: события вью придут дважды",
    );
    assert_eq!(first.connections.load(Ordering::SeqCst), 1);

    // Уборка общего слота клиента для соседних тестов: сайдкар «перешёл» на
    // другой хост, поэтому цепочка повторов второго клиента останавливается.
    state.set(HostEndpoint {
        port: 0,
        token: "gone".into(),
    });
    second.kill();
    let deadline = Instant::now() + WAIT;
    while crate::host_link::client().is_some() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(crate::host_link::client().is_none());
}
