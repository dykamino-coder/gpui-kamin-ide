//! Nth child tests for dom; split out to keep the owning module within 250 lines.

use crate::style::computed::Computed;

use super::*;

fn spans(nodes: &[Node], out: &mut Vec<Computed>) {
    for n in nodes {
        if let Node::Element(e) = n {
            if e.tag == "span" {
                out.push(e.style.clone());
            }
            spans(&e.children, out);
        }
    }
}

/// `:nth-child(1)` адресует ПЕРВОГО ребёнка — на нём держатся эталоны
/// целого набора тестов гибкой раскладки.
#[test]
fn first_child_is_addressable() {
    let html = r#"<style>
            span { background: white }
            span:nth-child(1) { background: yellow }
            span:first-child { color: red }
        </style><div style="display:flex"><span>a</span><span>b</span><span>c</span></div>"#;
    let mut found = vec![];
    spans(&parse(html, ""), &mut found);
    assert_eq!(found.len(), 3, "три куска");
    let first = &found[0];
    assert_ne!(
        first.background, found[1].background,
        "фон первого куска обязан отличаться от второго"
    );
    assert!(first.color.is_some(), ":first-child тоже обязан сработать");
}
