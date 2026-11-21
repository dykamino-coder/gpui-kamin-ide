//! Each record point of the DevTools `why` journal on a document edit names
//! its stable code at the engine line that decided: the frame's misses and
//! its unpublished record, the paint path's refusals, a box the layer does
//! not own, a layer that was not made and a text edit off the plan. Every
//! code recorded belongs to the protocol (`why::is_code`).

use super::reuse_tests::{settled, update};
use super::tests::frame;
use crate::why::Cause;
use gpui::TestAppContext;

/// `body` under the rules `css`; `class` goes where `CLASS` stands.
fn page(css: &str, body: &str, class: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         p{{width:120px;font:16px/20px sans-serif;margin:0}} {css}</style>\
         </head><body>{}</body></html>",
        body.replace("CLASS", class)
    )
}

/// The causes the journal recorded for the edit of a settled document from
/// `CLASS` = `""` to `class` and its frame.
fn causes(cx: &mut TestAppContext, css: &str, body: &str, class: &str) -> Vec<Cause> {
    let at = |class: &str| page(css, body, class);
    let (view, vcx) = settled(cx, &at(""), &at("x"));
    update(&view, vcx, &at(""));
    update(&view, vcx, &at(""));
    let (_one, _sub) = crate::why::test_reader();
    vcx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&at(class), "").unwrap()));
    frame(vcx);
    let causes = crate::why::take().journal.causes;
    for c in &causes {
        assert!(
            crate::why::is_code(c.code),
            "{} is no protocol code",
            c.code
        );
    }
    causes
}

/// The cause `code` with its site under `file`.
fn find<'a>(causes: &'a [Cause], code: &str, file: &str) -> &'a Cause {
    let cause = causes
        .iter()
        .find(|c| c.code == code)
        .unwrap_or_else(|| panic!("no {code} in {causes:#?}"));
    assert!(cause.site().contains(file), "{code} at {}", cause.site());
    cause
}

#[gpui::test]
fn a_built_frame_names_its_miss_and_the_unpublished_record(cx: &mut TestAppContext) {
    // A new box moves the rest: the revision builds its frame.
    let causes = causes(cx, "", "<p>one</p>CLASS<p>two</p>", "<p>new</p>");
    let miss = causes
        .iter()
        .find(|c| c.code.starts_with("frame_miss."))
        .unwrap_or_else(|| panic!("no frame miss in {causes:#?}"));
    assert!(
        miss.site().contains("crates/html/src/document/retained/"),
        "{}",
        miss.site()
    );
}

#[gpui::test]
fn a_refused_paint_path_names_the_refusal(cx: &mut TestAppContext) {
    let css = "#s{border:2px solid #00f;padding:1px} #s.b{background:#eee}";
    let body = "<p>one <span id=s class='CLASS'>two three four five six seven</span> end</p>";
    let causes = causes(cx, css, body, "b");
    find(&causes, "paint_path.refused_by_layer", "crates/html/src/");
}

#[gpui::test]
fn a_box_without_a_painter_in_the_layer_is_not_own_kind(cx: &mut TestAppContext) {
    let causes = causes(
        cx,
        "#s.b{background:#eee}",
        "<div id=s class='CLASS' style='height:20px'></div>",
        "b",
    );
    let cause = find(&causes, "not_own.kind", "document/revision/layer_write.rs");
    assert!(cause.detail.contains("<div>"), "{}", cause.detail);
}

#[gpui::test]
fn a_paint_the_layer_cannot_take_is_not_own(cx: &mut TestAppContext) {
    let css = "#s{background:#eee} #s.b{box-shadow:0 0 4px #000}";
    let causes = causes(
        cx,
        css,
        "<div id=s class='CLASS' style='height:20px'></div>",
        "b",
    );
    find(&causes, "not_own", "document/revision/layer_write.rs");
}

#[gpui::test]
fn a_frame_without_a_layer_names_the_layer_refusal(cx: &mut TestAppContext) {
    // A composition group (opacity) makes the frame splice its scene: no layer.
    let css = "#g{opacity:.5} #s{background:#eee} #s.b{background:#0af}";
    let body = "<div id=g>group</div><div id=s class='CLASS' style='height:20px'></div>";
    let causes = causes(cx, css, body, "b");
    let cause = causes
        .iter()
        .find(|c| c.code.starts_with("no_layer."))
        .unwrap_or_else(|| panic!("no no_layer cause in {causes:#?}"));
    assert!(cause.site().contains(".rs:"), "{}", cause.site());
}

#[gpui::test]
fn a_text_edit_off_the_plan_names_why(cx: &mut TestAppContext) {
    // The old text is blank: the edit is no text write.
    let causes = causes(cx, "", "<p id=t>CLASS</p>", "word");
    let cause = find(&causes, "plan_text.refused", "document/revision/adopt.rs");
    assert!(!cause.detail.is_empty());
}
