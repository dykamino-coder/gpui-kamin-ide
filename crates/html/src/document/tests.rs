//! Тесты `Document`: `html::after` переживает снятие обёрток, та же разметка
//! не разбирается заново, смена темы вызывает разбор, обёртки `<html>`/`<body>`
//! снимаются до блоков верхнего уровня, счёт узлов идёт вглубь.

use crate::dom::Node;

use super::*;

#[test]
fn html_after_survives_unwrap() {
    let doc = Document::builder("<style>html::after { content: \"AFTER\"; display: block; }</style>             <body><div>x</div></body>", "").build();
    let tags: Vec<String> = doc
        .nodes()
        .iter()
        .map(|n| match n {
            Node::Element(e) => e.tag.clone(),
            Node::Text(t) => format!("txt[{t}]"),
        })
        .collect();
    fn has_after(nodes: &[Node]) -> bool {
        nodes.iter().any(|n| match n {
            Node::Element(e) => e.tag.contains("after") || has_after(&e.children),
            _ => false,
        })
    }
    assert!(has_after(doc.nodes()), "html::after потерян: {tags:?}");
}

#[test]
fn same_markup_is_not_reparsed() {
    let mut doc = Document::builder("<p>раз</p>", "").build();
    assert!(
        !doc.update("<p>раз</p>", ""),
        "разметка та же — разбора нет"
    );
    assert!(
        doc.update("<p>два</p>", ""),
        "разметка иная — разобрали заново"
    );
}

#[test]
fn theme_change_also_triggers_a_reparse() {
    let mut doc = Document::builder("<p>раз</p>", "").build();
    assert!(
        doc.update("<p>раз</p>", "p { color: red }"),
        "сменилась тема"
    );
}

#[test]
fn standards_document_wrappers_are_unwrapped() {
    // Парсер всегда добавляет <html><body>; для виртуализации нужны
    // настоящие блоки документа, а не одна обёртка.
    let doc = Document::builder("<!DOCTYPE html><div>раз</div><div>два</div>", "").build();
    assert_eq!(
        doc.top_level_blocks(),
        2,
        "получено {}",
        doc.top_level_blocks()
    );
}

#[test]
fn quirks_document_wrappers_retain_their_fill_boxes() {
    let doc = Document::builder("<div>раз</div><div>два</div>", "").build();
    assert_eq!(doc.top_level_blocks(), 1);
}

#[test]
fn node_count_walks_the_whole_tree() {
    let doc = Document::builder("<div><p>раз</p><p>два</p></div>", "").build();
    // html + body + div + два абзаца + два текста — важна не точная цифра,
    // а то, что счёт идёт вглубь.
    assert!(doc.node_count() >= 5, "получено {}", doc.node_count());
}
