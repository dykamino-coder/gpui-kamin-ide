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
        page.set_journal(journal.clone(), window, cx);
        *output.borrow_mut() = Some(journal);
        page
    });
    frame(cx);
    frame(cx);
    frame(cx);
    let journal = saved.borrow().as_ref().unwrap().clone();
    let out = journal.to_json();
    assert_eq!(out["schema"], 1);
    #[cfg(feature = "heap-report")]
    {
        assert_eq!(out["owner_walks"], 1, "redraws must not walk owners");
        assert_eq!(out["memory_samples"][0]["owner_walks"], 1);
    }
    let frames = out["frames"].as_array().unwrap();
    assert!(!frames.is_empty(), "deferred frame observation was lost");
    let cold = &frames[0];
    assert_eq!(cold["document_step_calls"], 0);
    assert_eq!(cold["built"], 1);
    assert_eq!(cold["builds_inside_draw"], 1);
    let roots = cold["document_roots"]["attempts"].as_array().unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0]["prepared"], false);
    assert_eq!(roots[0]["painted"], true);
    for phase in ["build", "request_layout", "prepaint", "paint"] {
        assert!(roots[0]["phases_ms"][phase]["calls"].as_u64().unwrap() > 0);
    }
    let warm = frames.last().unwrap();
    assert_eq!(warm["document_step_calls"], 1);
    assert_eq!(warm["built"], 0);
    assert_eq!(warm["replayed"], 1);
    assert_eq!(warm["builds_inside_draw"], 0);
    assert!(warm["phases_ms"]["document_step"].is_number());
    assert!(warm["phases_ms"]["journal_memory"].is_number());
    assert!(warm["phases_ms"]["journal_record"].is_number());
    assert_eq!(warm["memory"]["sampled"], false);
    assert_eq!(warm["memory"]["sample_id"], 1);
    assert!(warm["memory"].get("owners").is_none());
    let samples = out["memory_samples"].as_array().unwrap();
    assert_eq!(samples.len(), 1, "redraws must not sample memory owners");
    assert_eq!(samples[0]["reason"], "open");
    assert_eq!(samples[0]["after_frame"], 0);
    assert!(samples[0]["duration_ms"].is_number());
    #[cfg(feature = "heap-report")]
    assert!(samples[0]["heap_bytes"].is_number());
    assert!(warm["causes"].is_array());
    cx.update(|window, cx| {
        page.update(cx, |page, cx| {
            page.document
                .update(&format!("{LONG}{}", "<p>new node</p>".repeat(100)), "")
                .unwrap();
            cx.notify();
        });
        journal.sample_memory("request", page.read(cx).document(), window, cx);
    });
    let requested = journal.to_json();
    let samples = requested["memory_samples"].as_array().unwrap();
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[1]["reason"], "request");
    #[cfg(feature = "heap-report")]
    {
        assert_eq!(requested["owner_walks"], 2);
        assert_eq!(samples[1]["owner_walks"], 1);
    }
    assert_eq!(samples[1]["after_frame"], frames.len());
    #[cfg(feature = "heap-report")]
    assert_ne!(
        samples[0]["owners"]["dom/nodes"], samples[1]["owners"]["dom/nodes"],
        "an explicit snapshot must observe the mutated document"
    );
    frame(cx);
    let after = journal.to_json();
    #[cfg(feature = "heap-report")]
    assert_eq!(after["owner_walks"], 2, "mutation redraw walked owners");
    assert_eq!(after["memory_samples"].as_array().unwrap().len(), 2);
    assert_eq!(
        after["frames"].as_array().unwrap().last().unwrap()["memory"]["sample_id"],
        2
    );
    // Drop every handle before releasing the reader-test serialization guard.
    cx.update(|_, cx| page.update(cx, |page, _| page.journal = None));
    drop(journal);
    drop(saved);
    assert!(!crate::why::enabled());
}
