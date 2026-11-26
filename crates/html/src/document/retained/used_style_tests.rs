//! The box tree keeps one used style per computed style and role for the
//! whole document, not a copy per box and frame (`document::boxes::used`).

use super::tests::{frame, open};
use gpui::TestAppContext;

#[gpui::test]
fn line_atoms_of_one_style_share_one_spacer_style(cx: &mut TestAppContext) {
    let row = "<td><span>a b</span></td>".repeat(8);
    let html = format!(
        "<style>span{{display:inline-block}}</style>\
         <table><tr>{row}</tr><tr>{row}</tr></table>"
    );
    let (view, cx) = open(cx, &html);
    frame(cx);
    let spacers = cx.update(|_, cx| view.read(cx).doc.state.boxes.borrow().used.len());
    assert!(
        (1..=2).contains(&spacers),
        "16 atoms in 16 cells of one style kept {spacers} spacer styles"
    );
    frame(cx);
    let again = cx.update(|_, cx| view.read(cx).doc.state.boxes.borrow().used.len());
    assert_eq!(
        again, spacers,
        "a later frame derived its own spacer styles"
    );
}
