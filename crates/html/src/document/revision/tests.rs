//! Ревизия без перерасчёта стиля равна свежей сборке той же разметки и выбирается
//! только тогда, когда отличие не может изменить каскад.

use crate::document::Document;
use std::rc::Rc;

/// Дерево коробок и стиль корня; ручки арен сравниваются без номера области,
/// который у каждого `DocumentValues` свой.
fn tree(doc: &Document) -> String {
    let text = format!("{:?}\n{:?}", doc.nodes(), doc.root_style());
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(i) = rest.find("owner: ") {
        out.push_str(&rest[..i + 7]);
        rest = rest[i + 7..].trim_start_matches(|c: char| c.is_ascii_digit());
        out.push('_');
    }
    out.push_str(rest);
    out
}

/// Обновить `old` до `new`: принята ли ревизия без каскада, и равна ли она свежей сборке.
fn update(old: &str, new: &str, theme: &str) -> bool {
    let mut doc = Document::builder(old, theme).build().unwrap();
    let values = doc.state.values.clone();
    assert!(doc.update(new, theme).unwrap().changed());
    let fresh = Document::builder(new, theme).build().unwrap();
    assert_eq!(tree(&doc), tree(&fresh), "{old} -> {new}");
    assert_eq!(doc.salt(), fresh.salt());
    assert_eq!(state(&doc), state(&fresh), "{old} -> {new}");
    Rc::ptr_eq(&values, &doc.state.values)
}

/// Факты `DocState`, которые пишет сборка дерева: режим, метрики корня,
/// at-правила `@page`, диагностики, число адресуемых исходных элементов и
/// номера прочитанных сборкой исходных текстов.
fn state(doc: &Document) -> String {
    let values = &doc.state.values;
    let elements = values.traversal.elements();
    let texts = values.traversal.texts();
    let read = values.traversal.read_texts();
    let read: Vec<usize> = (0..texts.len())
        .filter(|&i| read.contains(&texts[i]))
        .collect();
    format!(
        "{} {:?} {} {:?} {} {} {:?} {}",
        values.quirks(),
        values.root.borrow(),
        values.page_rules().len(),
        values.diagnostics_snapshot(),
        elements.len(),
        elements.iter().filter(|&&a| a != 0).count(),
        read,
        values.traversal.read_texts().len(),
    )
}

mod damage;
mod differential;
mod invalidation;
mod records;
mod repaint;
mod repaint_background;
mod revision_budget;
mod text_edits;

const PAGE: &str = "<!doctype html><html><head><style>p{color:#123}td{padding:1px}\
    .on{font-weight:bold}</style></head><body><p>a</p><div><p id=x>b<span>c</span></p>\
    <table><tr><td>1</td><td>2</td></tr></table></div><p>d</p></body></html>";

#[test]
fn unmentioned_data_attributes_and_classes_keep_the_cascade() {
    let edits = [
        PAGE.replace("<p id=x>", "<p id=x data-edit=\"1\">"),
        PAGE.replace("<html>", "<html class=\"alt\">"),
        PAGE.replace("<td>2", "<td class=\"qq rr\" data-a=\"1\" data-b=\"2\">2"),
        PAGE.replace("<span>", "<span class=\"on-x\">"),
    ];
    for new in &edits {
        assert!(update(PAGE, new, ""), "{new}");
        assert!(update(new, PAGE, ""), "{new}");
    }
    // Класс, совпадающий с именем элемента листа агента (`q`, `b`), не упомянут.
    let classed = PAGE.replace("<p>d", "<p class=\"a q\">d");
    assert!(update(
        &classed,
        &classed.replace("\"a q\"", "\"a b q\""),
        ""
    ));
}

#[test]
fn mentioned_names_and_other_changes_rebuild_the_cascade() {
    let d = |attrs: &str| PAGE.replace("<p>d", &format!("<p {attrs}>d"));
    let attr_style = "style=\"--n:attr(data-n)\"";
    let rebuilt = [
        // Класс, атрибут или `attr()` в листе документа, теме или своём `style`.
        (PAGE.to_string(), d("class=\"on\""), ""),
        (
            PAGE.to_string(),
            d("data-n=5"),
            "p::after{content:attr(data-n)}",
        ),
        (PAGE.to_string(), d("data-n=5"), "[DATA-N]{color:red}"),
        (d(attr_style), d(&format!("{attr_style} data-n=5")), ""),
        // Атрибут, который каскад читает сам, дети и комментарии.
        (PAGE.to_string(), PAGE.replace("<p id=x>", "<p id=y>"), ""),
        (
            PAGE.to_string(),
            PAGE.replace("<td>1", "<td colspan=2>1"),
            "",
        ),
        (
            PAGE.to_string(),
            PAGE.replace("<p>d</p>", "<p>d</p><p>e</p>"),
            "",
        ),
        (
            PAGE.to_string(),
            PAGE.replace("<p>a</p>", "<!-- c --><p>a</p>"),
            "",
        ),
    ];
    for (old, new, theme) in &rebuilt {
        assert!(!update(old, new, theme), "{new} {theme}");
    }
    // Тема ревизии другая: каскад зависит от неё.
    let mut doc = Document::builder(PAGE, "").build().unwrap();
    let values = doc.state.values.clone();
    doc.update(&d("data-q"), "p{color:red}").unwrap();
    assert!(!Rc::ptr_eq(&values, &doc.state.values));
}

#[test]
fn foreign_and_template_attributes_rebuild_the_cascade() {
    let page = "<body><svg><rect class=a width=5 height=5></rect></svg>\
        <template><p class=a>t</p></template><math><mi class=a>x</mi></math></body>";
    for (from, to) in [
        ("<rect class=a", "<rect class=b"),
        ("<p class=a>t", "<p class=b>t"),
        ("<mi class=a", "<mi class=b"),
    ] {
        assert!(!update(page, &page.replace(from, to), ""), "{to}");
    }
}

#[test]
fn adopted_revision_records_the_new_source_for_the_next_update() {
    let first = PAGE.replace("<p id=x>", "<p id=x data-k=\"1\">");
    let second = PAGE.replace("<p id=x>", "<p id=x data-k=\"2\">");
    let mut doc = Document::builder(PAGE, "").build().unwrap();
    let values = doc.state.values.clone();
    doc.update(&first, "").unwrap();
    doc.update(&second, "").unwrap();
    assert_eq!(data_k(&doc.source_dom()).as_deref(), Some("2"));
    doc.update(PAGE, "").unwrap();
    assert!(Rc::ptr_eq(&values, &doc.state.values));
    let fresh = Document::builder(PAGE, "").build().unwrap();
    assert_eq!(tree(&doc), tree(&fresh));
    assert_eq!(data_k(&doc.source_dom()), None);
}

/// Значение `data-k` первого элемента исходного DOM, у которого он есть.
fn data_k(node: &markup5ever_rcdom::Handle) -> Option<String> {
    if let markup5ever_rcdom::NodeData::Element { attrs, .. } = &node.data
        && let Some(a) = attrs.borrow().iter().find(|a| &*a.name.local == "data-k")
    {
        return Some(a.value.to_string());
    }
    node.children.borrow().iter().find_map(data_k)
}

#[test]
fn root_class_skips_the_viewport_box_that_shares_the_root_number() {
    // Политика прокрутки корня даёт коробку `html-viewport` с номером, следующим
    // за наибольшим номером дерева, — номером развёрнутого `html`.
    let page = "<!doctype html><html><head><style>html{scrollbar-color:red blue}\
        </style></head><body><div><div>a</div></div></body></html>";
    let classed = page.replace("<html>", "<html class=\"zz-alt\">");
    assert!(update(page, &classed, ""));
    assert!(update(&classed, page, ""));
}
