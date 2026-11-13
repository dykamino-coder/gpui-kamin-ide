//! Probe keystrokes use GPUI's main-window dispatch, never OS input or focus.

use gpui::{AnyWindowHandle, App, Keystroke};
use serde_json::{Value, json};
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, Instant};

static TX: OnceLock<smol::channel::Sender<Request>> = OnceLock::new();
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(crate) struct Request {
    key: Keystroke,
    deadline: Instant,
    reply: mpsc::Sender<Result<bool, String>>,
}

impl Request {
    pub(crate) fn dispatch(self, window: &mut gpui::Window, cx: &mut App) {
        let result = if Instant::now() >= self.deadline {
            Err("keystroke expired before UI dispatch".into())
        } else {
            Ok(window.dispatch_keystroke(self.key, cx))
        };
        let _ = self.reply.send(result);
    }
}

pub(crate) fn start(window: AnyWindowHandle, cx: &mut App) {
    TX.get_or_init(|| pump(window, cx));
}

fn pump(window: AnyWindowHandle, cx: &mut App) -> smol::channel::Sender<Request> {
    let (tx, rx) = smol::channel::unbounded::<Request>();
    cx.spawn(async move |cx| {
        while let Ok(request) = rx.recv().await {
            if window
                .update(cx, |_, window, cx| request.dispatch(window, cx))
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
    tx
}

fn request(key: &str) -> Result<(Request, mpsc::Receiver<Result<bool, String>>), String> {
    if key.is_empty() {
        return Err("key is required (GPUI syntax: enter, f2, ctrl-a)".into());
    }
    let key = Keystroke::parse(key).map_err(|err| err.to_string())?;
    let (reply, rx) = mpsc::channel();
    Ok((
        Request {
            key,
            deadline: Instant::now() + TIMEOUT,
            reply,
        },
        rx,
    ))
}

pub(crate) fn keystroke(req: &Value) -> Value {
    let result = (|| {
        let (request, reply) = request(req.get("key").and_then(Value::as_str).unwrap_or(""))?;
        TX.get()
            .ok_or("main window not ready")?
            .try_send(request)
            .map_err(|err| err.to_string())?;
        reply.recv_timeout(TIMEOUT).map_err(|err| err.to_string())?
    })();
    match result {
        Ok(handled) => json!({"ok": true, "handled": handled}),
        Err(err) => json!({"ok": false, "err": err}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        Context, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
        Styled as _, TestAppContext, VisualTestContext, div,
    };
    use std::sync::{Arc, Mutex};

    struct Keys {
        focus: FocusHandle,
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl Render for Keys {
        fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
            let seen = self.seen.clone();
            div()
                .size_full()
                .track_focus(&self.focus)
                .on_key_down(move |event, _, cx| {
                    seen.lock().unwrap().push(event.keystroke.key.clone());
                    cx.stop_propagation();
                })
                .child("keyboard fixture")
        }
    }

    #[gpui::test]
    fn enter_and_f2_reach_focused_gpui_element_and_acknowledge_dispatch(cx: &mut TestAppContext) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut queue = None;
        let view = cx.add_window(|window, cx| {
            queue = Some(pump(window.window_handle(), cx));
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            Keys {
                focus,
                seen: seen.clone(),
            }
        });
        let cx = VisualTestContext::from_window(*view, cx).into_mut();
        cx.run_until_parked();
        for key in ["enter", "f2"] {
            let (request, reply) = request(key).unwrap();
            queue.as_ref().unwrap().try_send(request).unwrap();
            cx.run_until_parked();
            assert_eq!(reply.try_recv().unwrap(), Ok(true));
        }
        assert_eq!(*seen.lock().unwrap(), ["enter", "f2"]);
        let (mut expired, reply) = request("enter").unwrap();
        expired.deadline = Instant::now() - Duration::from_secs(1);
        queue.as_ref().unwrap().try_send(expired).unwrap();
        cx.run_until_parked();
        assert!(reply.try_recv().unwrap().is_err());
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    #[test]
    fn parses_modifiers_and_reports_invalid_or_absent_keys() {
        let (request, _) = request("ctrl-shift-a").unwrap();
        assert!(request.key.modifiers.control && request.key.modifiers.shift);
        assert!(super::request("").is_err());
        assert!(super::request("not-a-modifier-enter").is_err());
    }
}
