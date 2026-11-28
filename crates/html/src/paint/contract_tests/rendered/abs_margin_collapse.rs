//! Out-of-flow boxes before the first block do not separate its top margin
//! from its parent's (CSS 2.1 §8.3.1): they create no line box.

use super::continuations::is;
use super::{quads, top};

#[test]
fn absolute_inline_with_text_lets_the_first_margin_collapse_through_body() {
    for probe in [
        "<span style='position:absolute'>x</span>",
        "<span style='position:absolute;left:300px;top:0'>x</span>",
    ] {
        let html = format!(
            "<style>body{{margin:8px}}</style>{probe}\
             <div style='margin-top:100px;height:20px;width:100px;background:#0000ff'></div>"
        );
        let found = quads(&html, 400., 300.);
        assert!(
            is(top(&found, 20., 101.), 0x0000ff),
            "{probe}: block at its margin"
        );
        assert!(
            !is(top(&found, 20., 121.), 0x0000ff),
            "{probe}: not pushed by body's margin"
        );
    }
}
