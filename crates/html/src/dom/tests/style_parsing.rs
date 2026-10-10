//! Style parsing for tests; split out to keep the owning module within 250 lines.

use super::*;
use super::{child_colors, first_element};

#[test]
pub(super) fn tag_defaults_apply() {
    let nodes = parse("<h1>Заголовок</h1>", "");
    let h1 = first_element(&nodes);
    assert_eq!(h1.tag, "h1");
    assert_eq!(h1.style.font_weight, Some(700));
}

#[test]
pub(super) fn inline_style_beats_stylesheet() {
    let nodes = parse(
        r#"<style>.c { color: red }</style><div class="c" style="color: #00ff00">x</div>"#,
        "",
    );
    let div = first_element(&nodes);
    assert_eq!(div.style.color.map(|c| c.g), Some(1.0));
}

#[test]
pub(super) fn descendant_selector_needs_the_ancestor() {
    let html = r#"<style>.card .t { color: #0000ff }</style>
            <div class="card"><span class="t">внутри</span></div><span class="t">снаружи</span>"#;
    let nodes = parse(html, "");
    let mut found = vec![];
    collect_spans(&nodes, &mut found);
    assert_eq!(found.len(), 2);
    assert_eq!(
        found[0].style.color.map(|c| c.b),
        Some(1.0),
        "внутри карточки — покрашен"
    );
    assert_eq!(
        found[1].style.color, None,
        "снаружи — правило не применяется"
    );
}

pub(super) fn collect_spans<'a>(nodes: &'a [Node], out: &mut Vec<&'a Element>) {
    for n in nodes {
        if let Node::Element(e) = n {
            if e.tag == "span" {
                out.push(e);
            }
            collect_spans(&e.children, out);
        }
    }
}

#[test]
pub(super) fn css_variables_are_substituted() {
    // На переменных построены все современные темы: без подстановки такое
    // объявление терялось молча.
    let nodes = parse(":root { --brand: #00ff00 } .b { color: var(--brand) }", "");
    let _ = &nodes;
    let nodes = parse(
        "<style>:root { --brand: #00ff00 } .b { color: var(--brand) }</style>             <div class=\"b\">текст</div>",
        "",
    );
    assert_eq!(first_element(&nodes).style.color.map(|c| c.g), Some(1.0));
}

#[test]
pub(super) fn variable_fallback_is_used_when_undefined() {
    let nodes = parse(
        "<style>.b { color: var(--missing, #0000ff) }</style><div class=\"b\">t</div>",
        "",
    );
    assert_eq!(first_element(&nodes).style.color.map(|c| c.b), Some(1.0));
}

#[test]
pub(super) fn hover_rules_form_a_separate_layer() {
    let nodes = parse(
        "<style>.b { color: #ffffff } .b:hover { color: #ff0000 }</style>             <div class=\"b\">кнопка</div>",
        "",
    );
    let d = first_element(&nodes);
    assert_eq!(d.style.color.map(|c| c.r), Some(1.0), "базовый цвет белый");
    assert_eq!(
        d.style.color.map(|c| c.g),
        Some(1.0),
        "и не покрашен наведением"
    );
    let hover = d.hover.as_ref().expect("слой наведения собран");
    assert_eq!(hover.color.map(|c| c.g), Some(0.0), "в наведении — красный");
}

#[test]
pub(super) fn no_hover_rules_means_no_layer() {
    let nodes = parse("<div class=\"b\">без наведения</div>", "");
    assert!(first_element(&nodes).hover.is_none());
}

#[test]
pub(super) fn script_and_style_content_is_dropped() {
    let nodes = parse(
        "<script>alert(1)</script><style>.a{}</style><p>текст</p>",
        "",
    );
    let p = first_element(&nodes);
    assert_eq!(p.tag, "p");
    assert!(matches!(p.children.first(), Some(Node::Text(t)) if t == "текст"));
}

#[test]
pub(super) fn declarative_shadow_scopes_styles_and_slots() {
    let red = crate::style::values::value::Color::parse("red");
    let green = crate::style::values::value::Color::parse("green");
    // Плоские дети хоста: `b` тени (её правило, не документное) и слот
    // (правило тени). Документное `b { red }` в тень не протекает.
    let colors = child_colors(
        "<style>b { color: red } slot { color: green }</style>             <div id=\"box\"><template shadowrootmode=\"open\"><style>b { color: green } slot { color: red }</style>             <b></b><slot></slot></template><i></i></div>",
    );
    assert_eq!(colors, vec![green, red]);
    // Распределённый ребёнок стилизуется таблицей ДОКУМЕНТА, не тени.
    let colors = child_colors(
        "<style>i { color: green }</style>             <div><template shadowrootmode=\"open\"><style>i { color: red }</style>             <slot id=\"box\"></slot></template><i></i></div>",
    );
    assert_eq!(colors, vec![green]);
    // `:host` красит хост, `:has-slotted` — слот с распределёнными.
    let colors = child_colors(
        "<div id=\"box\"><div><template shadowrootmode=\"open\"><style>:host { color: green } slot { color: red } :has-slotted { color: green }</style>             <slot></slot></template><i></i></div></div>",
    );
    assert_eq!(colors, vec![green]);
    // Обычный `<template>` как прежде не рисуется.
    let colors = child_colors("<div id=\"box\"><template><b></b></template><i></i></div>");
    assert_eq!(colors, vec![None]);
}

#[test]
pub(super) fn display_none_removes_the_subtree() {
    let nodes = parse(
        r#"<div style="display:none"><p>невидимо</p></div><p>видно</p>"#,
        "",
    );
    let first = first_element(&nodes);
    assert_eq!(first.tag, "p");
    assert!(matches!(first.children.first(), Some(Node::Text(t)) if t == "видно"));
}

#[test]
pub(super) fn unclosed_tags_are_recovered_by_the_parser() {
    let nodes = parse("<div><p>раз<p>два</div>", "");
    let div = first_element(&nodes);
    let ps = div
        .children
        .iter()
        .filter(|n| matches!(n, Node::Element(e) if e.tag == "p"))
        .count();
    assert_eq!(ps, 2, "html5ever закрывает <p> сам");
}
