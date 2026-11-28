//! Shared root observations for a plain host, prepared placement and DOM revisions.

use super::tests::{frame, open, open_without_step};
use gpui::TestAppContext;
use serde_json::Value;

const PAGE: &str = "<p id='p' style='margin:0'>one</p>";

fn painted(roots: &Value) -> &Value {
    roots["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|root| root["painted"] == true)
        .expect("no painted root")
}

fn phases(root: &Value) {
    for phase in ["build", "request_layout", "prepaint", "paint"] {
        assert!(
            root["phases_ms"][phase]["calls"].as_u64().unwrap() > 0,
            "{root}"
        );
        let ms = &root["phases_ms"][phase];
        assert!(ms["inclusive"].as_f64().unwrap() >= ms["exclusive"].as_f64().unwrap());
    }
    assert_eq!(root["pending_host_layout"], false);
    assert_eq!(
        root["phases_ms"]["host_layout_between_request_and_prepaint"]["calls"],
        1
    );
}

#[gpui::test]
fn root_phases_cover_cold_draw_prepared_layout_and_replay(cx: &mut TestAppContext) {
    let (_one, _reader) = crate::why::test_reader();
    let (view, cx) = open(cx, PAGE);
    let cold = cx.update(|_, cx| view.read(cx).doc.take_root_phases());
    let root = painted(&cold);
    phases(root);
    assert_eq!(root["prepared"], false);
    let identity = root["document"].clone();
    cx.update(|_, cx| view.update(cx, |view, _| view.viewport = (401., 300.)));
    frame(cx);
    let prepared = cx.update(|_, cx| view.read(cx).doc.take_root_phases());
    let root = painted(&prepared);
    phases(root);
    assert_eq!(root["document"], identity);
    assert_eq!(root["prepared"], true);
    assert!(
        root["phases_ms"]["layout_as_root"]["calls"]
            .as_u64()
            .unwrap()
            > 0
    );
    frame(cx);
    let replay = cx.update(|_, cx| view.read(cx).doc.take_root_phases());
    let root = painted(&replay);
    assert_eq!(root["document"], identity);
    assert_eq!(root["phases_ms"]["build"]["calls"], 0);
    assert_eq!(root["phases_ms"]["prepaint"]["calls"], 1);
    assert_eq!(root["phases_ms"]["paint"]["calls"], 1);
}

#[gpui::test]
fn host_without_a_document_step_uses_the_same_phase_protocol(cx: &mut TestAppContext) {
    let (_one, _reader) = crate::why::test_reader();
    let (view, cx) = open_without_step(cx, PAGE);
    let cold = cx.update(|_, cx| view.read(cx).doc.take_root_phases());
    phases(painted(&cold));
    let before = painted(&cold)["tree"].clone();
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            view.doc.update("<p>changed</p><p>extra</p>", "").unwrap();
        })
    });
    frame(cx);
    let changed = cx.update(|_, cx| view.read(cx).doc.take_root_phases());
    let root = painted(&changed);
    phases(root);
    assert_ne!(root["tree"], before);
    assert_eq!(root["prepared"], false);
    assert_eq!(root["document"], painted(&cold)["document"]);
    assert!(root["attempt"].as_u64().unwrap() > painted(&cold)["attempt"].as_u64().unwrap());
}
