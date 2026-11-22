//! Each record point of the DevTools `why` journal on a document edit names
//! its stable code at the engine line that decided: the frame's misses and
//! its unpublished record, the paint path's refusals, a box the layer does
//! not own, a layer that was not made and a text edit off the plan. Every
//! code recorded belongs to the protocol (`why::is_code`).
//!
//! `not_own.kind` has no case here: every style input of a box's painter
//! kind but `:hover` and decorations is a layout value, so a paint-only edit
//! keeps the kind it was recorded with (`revision::repaint` refuses the
//! rest first).

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
    causes_of(cx, |class| page(css, body, class), class)
}

/// `causes` of the pages `at(class)`.
fn causes_of(cx: &mut TestAppContext, at: impl Fn(&str) -> String, class: &str) -> Vec<Cause> {
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
fn a_paint_the_layer_cannot_take_is_not_own(cx: &mut TestAppContext) {
    // The root canvas takes `body`'s paint (css-backgrounds-3 §2.11.2): a
    // border colour edit of `body` repaints only, and the layer refuses it.
    let at = |class: &str| {
        format!(
            "<!doctype html><html><head><style>html{{background:#fff}} \
             body{{margin:0;border:2px solid #00f}} body.b{{border-color:#f00}} \
             p{{font:16px/20px sans-serif;margin:0}}</style></head>\
             <body class='{class}'><p>text</p></body></html>"
        )
    };
    let causes = causes_of(cx, at, "b");
    let cause = find(&causes, "not_own", "document/revision/layer_write.rs");
    assert!(
        cause.detail.starts_with("root_canvas") && cause.detail.contains("<body>"),
        "{}",
        cause.detail
    );
}
#[gpui::test]
fn a_frame_without_a_layer_names_the_layer_refusal(cx: &mut TestAppContext) {
    // A plane of a 3D context is a group the layer cannot keep (its
    // projection is the frame's), so the frame splices its scene and keeps
    // no layer (`gpui::Window::layer_keeps_groups`).
    let css = "#p{perspective:100px} #r{transform:rotateY(20deg);opacity:.5} \
               #s{background:#eee} #s.b{background:#0af}";
    let body =
        "<div id=p><div id=r>x</div></div><div id=s class='CLASS' style='height:20px'></div>";
    let causes = causes(cx, css, body, "b");
    let cause = find(&causes, "no_layer.spliced", "window/scene_layer.rs");
    assert!(cause.detail.contains("spliced"), "{}", cause.detail);
}

#[gpui::test]
fn a_text_edit_off_the_plan_names_why(cx: &mut TestAppContext) {
    // The old text is blank by CSS (a lone space): the edit is no text write.
    let causes = causes(cx, "", "<p id=t> CLASS</p>", "word");
    let cause = find(&causes, "plan_text.refused", "document/revision/adopt.rs");
    assert!(!cause.detail.is_empty());
}
