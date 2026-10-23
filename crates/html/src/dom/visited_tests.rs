//! Ограничение `:visited` (Selectors 4 §8.1): правила посещённой ссылки
//! задают только цвета, правила `:link` действуют на посещённую ссылку
//! всем, кроме этих цветов.

use super::*;
use crate::style::computed::Computed;
use crate::style::values::value::{Color, Len};

fn styles(nodes: &[Node], tag: &str, out: &mut Vec<Computed>) {
    for n in nodes {
        if let Node::Element(e) = n {
            if e.tag == tag {
                out.push(e.style.cascaded().clone());
            }
            styles(&e.children, tag, out);
        }
    }
}

fn first(html: &str, tag: &str) -> Computed {
    let mut out = vec![];
    styles(&parse(html, ""), tag, &mut out);
    out.remove(0)
}

fn green() -> Option<Color> {
    first(r#"<b style="color: rgb(0, 128, 0)">x</b>"#, "b").color
}

#[test]
fn visited_rule_sets_only_visited_dependent_colors() {
    let a = first(
        r#"<style>:visited { accent-color: red; font-size: 40px; color: rgb(0, 128, 0) }</style>
        <a href="">x</a>"#,
        "a",
    );
    assert_eq!(
        a.accent_color, None,
        "accent-color is not visited-dependent"
    );
    assert_ne!(a.font_size, Some(Len::Px(40.0)));
    assert_eq!(a.color, green());
}

#[test]
fn link_rule_applies_to_a_visited_link_except_its_colors() {
    let a = first(
        r#"<style>a:link { font-size: 30px; color: red } a:visited { color: rgb(0, 128, 0) }</style>
        <a href="">x</a>"#,
        "a",
    );
    assert_eq!(a.font_size, Some(Len::Px(30.0)));
    assert_eq!(a.color, green());
}

#[test]
fn visited_ancestor_is_the_relevant_link() {
    let span = first(
        r#"<style>a:visited span { color: rgb(0, 128, 0); font-size: 40px }</style>
        <a href=""><span>x</span></a>"#,
        "span",
    );
    assert_eq!(span.color, green());
    assert_ne!(span.font_size, Some(Len::Px(40.0)));
}

#[test]
fn all_in_a_visited_rule_covers_only_visited_dependent_properties() {
    let a = first(
        r#"<style>a:visited { color: red; font-size: 30px } a:visited { all: unset }
        a { font-size: 20px }</style>
        <a href="">x</a>"#,
        "a",
    );
    assert_eq!(a.font_size, Some(Len::Px(20.0)), "all stays off font-size");
    assert_eq!(a.color, None, "color: unset inherits");
}
