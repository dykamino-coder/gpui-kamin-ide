//! The shared journal observes the cold draw and subsequent prepared replay.

use super::tests::{LONG, frame};
use super::*;
use gpui::{TestAppContext, size};
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn ordinary_view_has_no_recorder_or_deferred_journal_work(cx: &mut TestAppContext) {
    let (_one, reader) = crate::why::test_reader();
    drop(reader);
    assert!(!crate::why::enabled());
    let (page, cx) = super::tests::open(cx, LONG);
    frame(cx);
    cx.update(|_, cx| assert!(page.read(cx).journal.is_none()));
    assert!(
        !crate::why::enabled(),
        "drawing enabled the journal implicitly"
    );
    assert_eq!(crate::why::take(), crate::why::Entry::default());
}

#[gpui::test]
fn journal_records_document_steps_and_inside_draw_without_a_host_meter(cx: &mut TestAppContext) {
    let (_one, reader) = crate::why::test_reader();
    drop(reader);
    let saved = Rc::new(RefCell::new(None));
    let output = saved.clone();
    let (page, cx) = cx.add_window_view(move |window, cx| {
        let journal = crate::why::WindowJournal::new(window, cx);
        let document = Document::builder(LONG, "").build_for_view(true).unwrap();
        let mut page = DocumentView::new(document, size(px(400.), px(300.)), window, cx);
        page.set_journal(journal.clone());
        *output.borrow_mut() = Some(journal);
        page
    });
    frame(cx);
    frame(cx);
    frame(cx);
    let journal = saved.borrow().as_ref().unwrap().clone();
    let out = journal.to_json();
    assert_eq!(out["schema"], 1);
    let frames = out["frames"].as_array().unwrap();
    assert!(!frames.is_empty(), "deferred frame observation was lost");
    let cold = &frames[0];
    assert_eq!(cold["document_step_calls"], 0);
    assert_eq!(cold["built"], 1);
    assert_eq!(cold["builds_inside_draw"], 1);
    let warm = frames.last().unwrap();
    assert_eq!(warm["document_step_calls"], 1);
    assert_eq!(warm["built"], 0);
    assert_eq!(warm["replayed"], 1);
    assert_eq!(warm["builds_inside_draw"], 0);
    assert!(warm["phases_ms"]["document_step"].is_number());
    assert!(warm["phases_ms"]["journal_memory"].is_number());
    assert!(warm["phases_ms"]["journal_record"].is_number());
    assert!(warm["memory"]["heap_bytes"].is_number());
    assert!(warm["causes"].is_array());
    // Drop every handle before releasing the reader-test serialization guard.
    cx.update(|_, cx| page.update(cx, |page, _| page.journal = None));
    drop(journal);
    drop(saved);
    assert!(!crate::why::enabled());
}
