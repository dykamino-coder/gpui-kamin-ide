//! Probe file drags through GPUI's platform input, as the Windows drop target
//! delivers them, never OS input: a stand drops files on any view without a
//! real mouse or Explorer window.
//!
//! `{"cmd":"filedrop","paths":["C:\\a.txt"],"from":[x,y],"to":[x,y],"steps":6}`
//! enters at `from`, moves to `to` and drops; `"release":false` leaves the
//! window instead (the user dragged out).

use gpui::{AnyWindowHandle, App, ExternalPaths, FileDropEvent, InputEvent as _, Point, px};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{OnceLock, mpsc};
use std::time::Duration;

static TX: OnceLock<smol::channel::Sender<Request>> = OnceLock::new();
const TIMEOUT: Duration = Duration::from_secs(10);
/// One frame between steps: hover and drop targets come from the last frame.
const STEP: Duration = Duration::from_millis(40);

pub(crate) struct Request {
    paths: Vec<PathBuf>,
    points: Vec<Point<gpui::Pixels>>,
    release: bool,
    reply: mpsc::Sender<Result<(), String>>,
}

pub(crate) fn start(window: AnyWindowHandle, cx: &mut App) {
    TX.get_or_init(|| pump(window, cx));
}

fn events(request: &Request) -> Vec<FileDropEvent> {
    let mut out = Vec::new();
    let Some((first, rest)) = request.points.split_first() else {
        return out;
    };
    out.push(FileDropEvent::Entered {
        position: *first,
        paths: ExternalPaths(request.paths.iter().cloned().collect()),
    });
    out.extend(rest.iter().map(|p| FileDropEvent::Pending { position: *p }));
    let last = *request.points.last().unwrap_or(first);
    out.push(if request.release {
        FileDropEvent::Submit { position: last }
    } else {
        FileDropEvent::Exited
    });
    out.push(FileDropEvent::Ended);
    out
}

fn pump(window: AnyWindowHandle, cx: &mut App) -> smol::channel::Sender<Request> {
    let (tx, rx) = smol::channel::unbounded::<Request>();
    cx.spawn(async move |cx| {
        while let Ok(request) = rx.recv().await {
            let mut result = Ok(());
            for event in events(&request) {
                if window
                    .update(cx, |_, window, cx| {
                        window.dispatch_event(event.to_platform_input(), cx);
                        window.refresh();
                    })
                    .is_err()
                {
                    result = Err("main window closed".to_string());
                    break;
                }
                cx.background_executor().timer(STEP).await;
            }
            let _ = request.reply.send(result);
        }
    })
    .detach();
    tx
}

fn point(req: &Value, key: &str) -> Option<Point<gpui::Pixels>> {
    let a = req.get(key)?.as_array()?;
    Some(Point::new(
        px(a.first()?.as_f64()? as f32),
        px(a.get(1)?.as_f64()? as f32),
    ))
}

fn request(req: &Value) -> Result<(Request, mpsc::Receiver<Result<(), String>>), String> {
    let paths: Vec<PathBuf> = req
        .get("paths")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default();
    if paths.is_empty() {
        return Err("paths: [\"C:\\\\file\", ...] is required".into());
    }
    let (from, to) = match (point(req, "from"), point(req, "to")) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err("need from:[x,y] to:[x,y] (logical px of the window)".into()),
    };
    let steps = req
        .get("steps")
        .and_then(Value::as_u64)
        .unwrap_or(6)
        .clamp(1, 60) as usize;
    let points = (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
        })
        .collect();
    let (reply, rx) = mpsc::channel();
    Ok((
        Request {
            paths,
            points,
            release: req.get("release").and_then(Value::as_bool).unwrap_or(true),
            reply,
        },
        rx,
    ))
}

pub(crate) fn file_drop(req: &Value) -> Value {
    let result = (|| {
        let (request, reply) = request(req)?;
        TX.get()
            .ok_or("main window not ready")?
            .try_send(request)
            .map_err(|err| err.to_string())?;
        reply.recv_timeout(TIMEOUT).map_err(|err| err.to_string())?
    })();
    match result {
        Ok(()) => json!({"ok": true}),
        Err(err) => json!({"ok": false, "err": err}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drop_enters_moves_and_submits_at_the_target() {
        let (r, _) = request(
            &json!({"paths":["C:\\a.txt","C:\\b.png"],"from":[0,0],"to":[100,50],"steps":2}),
        )
        .unwrap();
        let ev = events(&r);
        assert!(matches!(&ev[0], FileDropEvent::Entered { paths, .. } if paths.paths().len() == 2));
        assert!(
            matches!(ev[1], FileDropEvent::Pending { position } if position == Point::new(px(50.0), px(25.0)))
        );
        assert!(
            matches!(ev[3], FileDropEvent::Submit { position } if position == Point::new(px(100.0), px(50.0)))
        );
        assert!(matches!(ev[4], FileDropEvent::Ended));
    }

    #[test]
    fn release_false_leaves_the_window_and_bad_requests_are_refused() {
        let (r, _) =
            request(&json!({"paths":["C:\\a"],"from":[0,0],"to":[1,1],"release":false})).unwrap();
        assert!(matches!(
            events(&r).iter().rev().nth(1),
            Some(FileDropEvent::Exited)
        ));
        assert!(request(&json!({"from":[0,0],"to":[1,1]})).is_err());
        assert!(request(&json!({"paths":["C:\\a"]})).is_err());
    }
}
