//! Тесты: селекторы предков работают через соседние поддеревья и сгенерированное содержимое.

use super::tree_parsing::{child_colors, pseudo_texts};
use crate::style::values::value::Color;

#[test]
fn completed_subtrees_do_not_leak_ancestors_into_siblings() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let html = r#"<style>
        #box > section > span { color: green; }
        #box > span { color: blue; }
        section span::before { content: 'inside'; }
        #box > span::before { content: 'outside'; }
        </style><div id=box><section><span>A</span></section><span>B</span></div>"#;
    let colors = child_colors(html);
    assert_eq!(colors[1], Color::parse("blue", vcx));
    assert_eq!(pseudo_texts(html), ["inside", "outside"]);
}
