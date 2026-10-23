//! Тест `white-space: break-spaces`: значение доезжает от разбора до правил
//! переноса абзаца (`break_spaces`, `keep_spaces`), от которых зависит учёт
//! хвостового пробела в ширине строки.

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
    assert_eq!(div.style.cascaded().break_after_spaces, Some(true), "разбор");
    assert!(whitespace::break_spaces(div.style.cascaded()), "правила переноса");
    assert!(whitespace::keep_spaces(div.style.cascaded()), "сохранение пробелов");
}
