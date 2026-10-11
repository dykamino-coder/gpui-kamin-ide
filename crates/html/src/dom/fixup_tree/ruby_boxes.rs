//! Ruby boxes for fixup_tree; split out to keep the owning module within 250 lines.

use crate::dom::*;
use crate::style::computed::{Computed, Position};

/// Сумма двух длин. Складываются только точки: смешивать доли и кегли здесь
/// не с чем — контейнера в этот момент нет.
/// Руби-роль коробки (css-ruby-1 §2.1): своё `display: ruby*`, иначе тег
/// без авторского `display` (A.1). Зеркало `render::ruby_role`.
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

/// css-ruby-1 §2.2 п.2: подряд идущие базы, аннотации и их контейнеры ВНЕ
/// руби-контейнера (вместе с пробелами между ними) оборачиваются в
/// анонимный руби-контейнер. Прежде `<rt>` прямо в `<p>` рисовалась мелким
/// строчным текстом в ряду (`ruby-box-generation-*`, вторая строка: эталон
/// пишет те же коробки внутри `<ruby>`). Краевые пробелы серии остаются
/// снаружи, строчное содержимое серию обрывает.
pub(crate) fn wrap_misparented_ruby(children: Vec<Node>) -> Vec<Node> {
    use crate::style::computed::RubyRole;
    let internal = |n: &Node| {
        matches!(n, Node::Element(e)
            if ruby_box_role(&e.tag, &e.style).is_some_and(|r| r != RubyRole::Container))
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
            out.push(Node::Element(Element {
                list_item: None,
                node_id: 0,
                anim: None,
                inline: true,
                tag: "ruby".to_string(),
                style: Computed::default(),
                hover: None,
                first_letter: None,
                first_line: None,
                children: std::mem::take(run),
                attrs: vec![],
            }));
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
