//! Руби-коробки при сборке дерева: роль и анонимный контейнер (css-ruby-1 §2).
//!
//! `ruby_box_role` — роль по `display: ruby*`, иначе по тегу без авторского
//! `display` (§2.1, A.1; зеркало `text::ruby::ruby_role`). `wrap_misparented_ruby`
//! (§2.2 п.2): подряд идущие базы, аннотации и их контейнеры вне руби-
//! контейнера оборачиваются в анонимный `<ruby>`; краевые пробелы остаются
//! снаружи, строчное содержимое серию обрывает. `own_containing_block` —
//! задаёт ли элемент отсчёт абсолютным потомкам.

use crate::dom::*;
use crate::style::computed::{Computed, Position};

/// Руби-роль коробки (css-ruby-1 §2.1): своё `display: ruby*`, иначе тег
/// без авторского `display` (A.1). Зеркало `text::ruby::ruby_role`.
pub(crate) fn ruby_box_role(
    tag: &str,
    style: &Computed,
) -> Option<crate::style::computed::RubyRole> {
    use crate::style::computed::RubyRole;
    if let Some(role) = style.ruby_role {
        return Some(role);
    }
    if style.display.is_some() {
        return None;
    }
    match tag {
        "ruby" => Some(RubyRole::Container),
        "rb" => Some(RubyRole::Base),
        "rt" => Some(RubyRole::Text),
        "rbc" => Some(RubyRole::BaseContainer),
        "rtc" => Some(RubyRole::TextContainer),
        _ => None,
    }
}

/// The inline ruby box has no authored identity; inheritance comes from its parent.
pub(crate) fn anonymous_ruby(children: Vec<Node>) -> Node {
    Node::Element(Element::anonymous(
        "ruby",
        Computed::default(),
        children,
        true,
        Marks::default(),
    ))
}

/// css-ruby-1 §2.2 п.2: подряд идущие базы, аннотации и их контейнеры ВНЕ
/// руби-контейнера (вместе с пробелами между ними) оборачиваются в
/// анонимный руби-контейнер. Прежде `<rt>` прямо в `<p>` рисовалась мелким
/// строчным текстом в ряду (`ruby-box-generation-*`, вторая строка: эталон
/// пишет те же коробки внутри `<ruby>`). Краевые пробелы серии остаются
/// снаружи, строчное содержимое серию обрывает.
pub(crate) fn wrap_misparented_ruby(
    children: Vec<Node>,
    parent: Option<crate::style::computed::RubyRole>,
) -> Vec<Node> {
    use crate::style::computed::RubyRole;
    let internal = |n: &Node| {
        matches!(n, Node::Element(e) if ruby_box_role(&e.tag, &e.style).is_some_and(|r| {
            match r {
                RubyRole::Container => false,
                RubyRole::Base => !matches!(parent, Some(RubyRole::Container | RubyRole::BaseContainer)),
                RubyRole::Text => !matches!(parent, Some(RubyRole::Container | RubyRole::TextContainer)),
                _ => parent != Some(RubyRole::Container),
            }
        }))
    };
    if !children.iter().any(internal) {
        return children;
    }
    let blank = |n: &Node| matches!(n, Node::Text(t) if t.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\x0c')));
    let mut out: Vec<Node> = Vec::with_capacity(children.len());
    let mut run: Vec<Node> = Vec::new();
    // Пробелы после последней руби-коробки серии: войдут в серию, только
    // если за ними снова руби-коробка.
    let mut pending: Vec<Node> = Vec::new();
    let flush = |run: &mut Vec<Node>, out: &mut Vec<Node>| {
        if !run.is_empty() {
            out.push(anonymous_ruby(std::mem::take(run)));
        }
    };
    for n in children {
        if internal(&n) {
            run.append(&mut pending);
            run.push(n);
        } else if blank(&n) && !run.is_empty() {
            pending.push(n);
        } else {
            flush(&mut run, &mut out);
            out.append(&mut pending);
            out.push(n);
        }
    }
    flush(&mut run, &mut out);
    out.append(&mut pending);
    out
}

/// Задаёт ли элемент отсчёт для абсолютных потомков.
pub(crate) fn own_containing_block(c: &Computed) -> bool {
    matches!(
        c.position,
        Some(Position::Relative)
            | Some(Position::Absolute)
            | Some(Position::Fixed)
            | Some(Position::Sticky)
    )
}

#[cfg(test)]
mod tests;
