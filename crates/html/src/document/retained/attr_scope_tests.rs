//! A `data-*` edit on a `display: contents` wrapper whose new rule gives it a
//! background: the wrapper generates no box (css-display-3 §2.5), so the
//! background reaches no pixel and the revision keeps its frame
//! (`Plan::Restyled` with nothing to write), however many elements the page
//! has. Blink gives such an element no `LayoutObject` to invalidate. The
//! wrappers sit under `:has()` and structural rules, as on the benchmark's
//! `selectors-*` pages.

use super::box_repaint_tests::{frames, quads};
use super::reuse_tests::{settled, update};
use super::tests::{frame, open};
use gpui::TestAppContext;

/// `sections` wrappers of 24 cells under `:has()` rules; the middle wrapper
/// carries `attr` (empty: none).
fn page(sections: usize, attr: &str) -> String {
    let mut css = String::from(
        "html{background:white}body{margin:0}.a{display:contents}\
         .cell{display:inline-block;width:8px;height:8px;background:#456}\
         .a:has(.chosen) .cell:nth-child(2n+1){background:#a32}",
    );
    for k in 0..24 {
        css.push_str(&format!(
            ".a:has(.key{k}) .cell:nth-child({}n+1){{border-left:1px solid rgb({k},30,90)}}",
            k % 13 + 2
        ));
    }
    css.push_str("[data-edit]{background:#eee}");
    let mut body = String::new();
    for s in 0..sections {
        let at = if s == sections / 2 { attr } else { "" };
        body.push_str(&format!("<section class=\"a\"{at}>"));
        for k in 0..24 {
            let chosen = if k == 15 { " chosen" } else { "" };
            body.push_str(&format!("<i class=\"cell key{k}{chosen}\"></i>"));
        }
        body.push_str("</section>");
    }
    format!(
        "<!doctype html><html><head><style>{css}</style></head><body>{body}\
         <div data-deep>end</div></body></html>"
    )
}

/// Toggle `data-edit` on the middle wrapper of a settled page: whether the
/// frame was kept, the edited frame and a fresh document's.
fn toggle(cx: &mut TestAppContext, sections: usize) -> (bool, Vec<String>, Vec<String>) {
    let (none, set) = (page(sections, ""), page(sections, " data-edit=\"1\""));
    let (view, vcx) = settled(cx, &none, &set);
    update(&view, vcx, &none);
    update(&view, vcx, &set);
    update(&view, vcx, &none);
    let before = frames(&view, vcx);
    vcx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&set, "").unwrap()));
    frame(vcx);
    let kept = frames(&view, vcx) == (before.0, before.1 + 1);
    let edited = quads(vcx);
    let (_, fresh) = open(cx, &set);
    frame(fresh);
    (kept, edited, quads(fresh))
}

#[gpui::test]
fn a_background_on_a_contents_wrapper_keeps_the_frame(cx: &mut TestAppContext) {
    let (kept, edited, fresh) = toggle(cx, 4);
    assert!(kept, "the frame was not kept");
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
}

#[gpui::test]
fn a_background_on_a_contents_wrapper_keeps_the_frame_of_a_larger_page(cx: &mut TestAppContext) {
    let (kept, edited, fresh) = toggle(cx, 32);
    assert!(kept, "the frame was not kept with 32 wrappers");
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
}

#[gpui::test]
fn a_contents_wrapper_whose_child_inherits_its_background_repaints(cx: &mut TestAppContext) {
    // The child takes the wrapper's reset background with `inherit`
    // (css-cascade-5 §7.3.1): the frame must show it.
    let page = |attr: &str| {
        format!(
            "<!doctype html><html><head><style>html,body{{margin:0;background:#fff}}\
             .a{{display:contents}}b{{display:block;width:20px;height:20px;background:inherit}}\
             [data-edit]{{background:#0a0}}</style></head><body>\
             <section class=a{attr}><b></b></section></body></html>"
        )
    };
    let (none, set) = (page(""), page(" data-edit=1"));
    let (view, vcx) = settled(cx, &none, &set);
    update(&view, vcx, &none);
    update(&view, vcx, &set);
    let edited = quads(vcx);
    let (_, fresh) = open(cx, &set);
    frame(fresh);
    assert_eq!(
        edited,
        quads(fresh),
        "the edited frame differs from a fresh one"
    );
}
