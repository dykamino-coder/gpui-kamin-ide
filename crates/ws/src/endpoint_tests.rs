//! Unit-тесты корреляционного ядра (plan/96 §1.1 ws-protocol).

use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::endpoint::Endpoint;
use crate::frame::RpcFrame;

/// Эндпоинт + приёмник кадров + журнал вызовов хендлера.
type TestRig = (
    Endpoint,
    std::sync::mpsc::Receiver<RpcFrame>,
    Arc<Mutex<Vec<(String, Value)>>>,
);

fn make_endpoint() -> TestRig {
    let (tx, rx) = channel();
    let events: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
    let ev = events.clone();
    let ep = Endpoint::new(
        tx,
        Arc::new(move |ch: &str, p: &Value| ev.lock().unwrap().push((ch.into(), p.clone()))),
        Arc::new(|_id: u64, method: &str, _params: &[Value]| match method {
            "shell.readClipboard" => crate::endpoint::HostReply::Now(Ok(json!("clip"))),
            other => crate::endpoint::HostReply::Now(Err(format!("unknown method: {other}"))),
        }),
    );
    (ep, rx, events)
}

#[test]
fn request_response_correlation() {
    let (ep, outbox, _) = make_endpoint();
    let rx1 = ep.request("a", vec![]);
    let rx2 = ep.request("b", vec![json!(1)]);
    // Ответы в обратном порядке — корреляция по id, не по порядку
    ep.dispatch(RpcFrame::Res {
        id: 2,
        ok: true,
        value: Some(json!("B")),
        error: None,
    });
    ep.dispatch(RpcFrame::Res {
        id: 1,
        ok: true,
        value: Some(json!("A")),
        error: None,
    });
    assert_eq!(rx1.recv().unwrap().unwrap(), json!("A"));
    assert_eq!(rx2.recv().unwrap().unwrap(), json!("B"));
    // Оба req ушли в outbox с растущими id
    let f1 = outbox.recv().unwrap();
    let f2 = outbox.recv().unwrap();
    assert!(matches!(f1, RpcFrame::Req { id: 1, .. }));
    assert!(matches!(f2, RpcFrame::Req { id: 2, .. }));
}

#[test]
fn error_response_maps_to_err() {
    let (ep, _outbox, _) = make_endpoint();
    let rx = ep.request("x", vec![]);
    ep.dispatch(RpcFrame::Res {
        id: 1,
        ok: false,
        value: None,
        error: Some("boom".into()),
    });
    assert_eq!(rx.recv().unwrap().unwrap_err(), "boom");
}

#[test]
fn events_reach_listener() {
    let (ep, _outbox, events) = make_endpoint();
    ep.dispatch(RpcFrame::Evt {
        channel: "sessions:changed".into(),
        payload: json!({"n": 1}),
    });
    let got = events.lock().unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, "sessions:changed");
}

#[test]
fn host_request_gets_response_frame() {
    let (ep, outbox, _) = make_endpoint();
    ep.dispatch(RpcFrame::Req {
        id: 9,
        method: "shell.readClipboard".into(),
        params: vec![],
    });
    match outbox.recv().unwrap() {
        RpcFrame::Res { id, ok, value, .. } => {
            assert_eq!(id, 9);
            assert!(ok);
            assert_eq!(value.unwrap(), json!("clip"));
        }
        other => panic!("expected res, got {other:?}"),
    }
}

#[test]
fn unknown_host_request_answers_error_like_rpc_ts() {
    // rpc.ts: {ok:false, error:"unknown method: X"} — не молчание
    let (ep, outbox, _) = make_endpoint();
    ep.dispatch(RpcFrame::Req {
        id: 3,
        method: "nope".into(),
        params: vec![],
    });
    match outbox.recv().unwrap() {
        RpcFrame::Res {
            id: 3,
            ok: false,
            error: Some(e),
            ..
        } => {
            assert!(e.contains("unknown method"));
        }
        other => panic!("expected error res, got {other:?}"),
    }
}

#[test]
fn fail_all_rejects_pending() {
    let (ep, _outbox, _) = make_endpoint();
    let rx = ep.request("hang", vec![]);
    ep.fail_all("kamin-host connection lost");
    assert_eq!(
        rx.recv().unwrap().unwrap_err(),
        "kamin-host connection lost"
    );
}
