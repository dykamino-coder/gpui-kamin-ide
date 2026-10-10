//! White space tests for dom; split out to keep the owning module within 250 lines.

use super::*;

/// `white-space: break-spaces` обязан доехать до правил переноса: от него
/// зависит, считается ли хвостовой пробел в ширину строки.
#[test]
fn break_spaces_reaches_the_wrap_rules() {
    let nodes = parse(r#"<div style="white-space: break-spaces">X XX X</div>"#, "");
    fn find(nodes: &[Node]) -> Option<&Element> {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.tag == "div" {
                    return Some(e);
                }
                if let Some(f) = find(&e.children) {
                    return Some(f);
                }
            }
        }
        None
    }
    let div = find(&nodes).expect("блок в дереве");
    assert_eq!(div.style.break_after_spaces, Some(true), "разбор");
    let wrap = crate::text::paragraph::rules(&div.style).expect("правила");
    assert!(wrap.break_spaces, "правила переноса");
    assert!(wrap.keep_spaces, "сохранение пробелов");
}
