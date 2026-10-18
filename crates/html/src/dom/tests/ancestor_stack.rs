//! Verify ancestor selectors across sibling subtrees and generated content.

use super::tree_parsing::{child_colors, pseudo_texts};
use crate::style::values::value::Color;

#[test]
fn completed_subtrees_do_not_leak_ancestors_into_siblings() {
    let html = r#"<style>
        #box > section > span { color: green; }
        #box > span { color: blue; }
        section span::before { content: 'inside'; }
        #box > span::before { content: 'outside'; }
        </style><div id=box><section><span>A</span></section><span>B</span></div>"#;
    let colors = child_colors(html);
    assert_eq!(colors[1], Color::parse("blue"));
    assert_eq!(pseudo_texts(html), ["inside", "outside"]);
}
