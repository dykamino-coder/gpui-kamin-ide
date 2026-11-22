//! A slot that does not display (CSS Scoping 1 §2.3: slots take part in the
//! flat tree like elements) displays none of the nodes assigned to it: a
//! closed dropdown in a shadow tree neither paints nor widens the overflow.

use super::tests::{frame, open};
use gpui::TestAppContext;

fn painted_wide(cx: &mut TestAppContext, rule: &str) -> usize {
    painted(cx, &host(rule))
}

/// A host whose shadow tree hides its `d` slot by `rule`.
fn host(rule: &str) -> String {
    format!(
        "<x-drop><template shadowrootmode=open>\
         <style>{rule}</style><slot name=b></slot><slot name=d></slot></template>\
         <button slot=b>x</button><div slot=d style='position:absolute;left:0;width:2000px;\
         height:50px;border:3px solid red'></div></x-drop>"
    )
}

fn painted(cx: &mut TestAppContext, body: &str) -> usize {
    let html = format!("<style>body{{margin:0}}</style>{body}");
    let mut app = cx.clone();
    let (_view, vcx) = open(&mut app, &html);
    frame(vcx);
    vcx.update(|window, _| {
        let scale = window.scale_factor();
        let quads = window.painted_quads();
        quads
            .iter()
            .filter(|q| q.bounds.size.width.0 / scale > 1500.0)
            .count()
    })
}

#[gpui::test]
fn a_slot_that_displays_none_hides_what_is_assigned_to_it(cx: &mut TestAppContext) {
    // `:not()` takes a selector list (selectors-4 §4.3), inside `:host()` too:
    // a closed dropdown's slot is hidden so.
    let failed: Vec<_> = [
        "slot[name=d]{display:none}",
        ":host(:not([loaded])) slot[name=d]{display:none}",
        ":host(:not([loaded],[x])) slot[name=d]{display:none}",
        ":host(:not([loaded],:focus-within)) slot[name=d]{display:none}",
    ]
    .into_iter()
    .filter(|rule| painted_wide(cx, rule) != 0)
    .collect();
    assert!(
        failed.is_empty(),
        "the closed slot's content painted: {failed:?}"
    );
    // The dropdown's host inside another host's shadow tree.
    let rule =
        ":host{display:contents}:host(:not([loaded],:focus-within)) slot[name=d]{display:none}";
    let nested = format!(
        "<x-theme><template shadowrootmode=open><div>{}</div></template></x-theme>",
        host(rule)
    );
    assert_eq!(
        painted(cx, &nested),
        0,
        "the nested closed slot's content painted"
    );
    assert!(painted_wide(cx, "") > 0, "an open slot painted nothing");
}
