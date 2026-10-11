//! Tests for document; split out to keep the owning module within 250 lines.

use crate::dom::Node;

use super::*;

#[test]
fn html_after_survives_unwrap() {
    let doc = Document::new(
        "<style>html::after { content: \"AFTER\"; display: block; }</style>             <body><div>x</div></body>",
        "",
    );
    let tags: Vec<String> = doc
        .nodes()
        .iter()
        .map(|n| match n {
            Node::Element(e) => e.tag.clone(),
            Node::Text(t) => format!("txt[{t}]"),
        })
        .collect();
    eprintln!("TOP: {tags:?}");
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
    let mut doc = Document::new("<p>раз</p>", "");
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
    let mut doc = Document::new("<p>раз</p>", "");
    assert!(
        doc.update("<p>раз</p>", "p { color: red }"),
        "сменилась тема"
    );
}

#[test]
fn document_wrappers_are_unwrapped() {
    // Парсер всегда добавляет <html><body>; для виртуализации нужны
    // настоящие блоки документа, а не одна обёртка.
    let doc = Document::new("<div>раз</div><div>два</div>", "");
    assert_eq!(
        doc.top_level_blocks(),
        2,
        "получено {}",
        doc.top_level_blocks()
    );
}

#[test]
fn node_count_walks_the_whole_tree() {
    let doc = Document::new("<div><p>раз</p><p>два</p></div>", "");
    // html + body + div + два абзаца + два текста — важна не точная цифра,
    // а то, что счёт идёт вглубь.
    assert!(doc.node_count() >= 5, "получено {}", doc.node_count());
}
