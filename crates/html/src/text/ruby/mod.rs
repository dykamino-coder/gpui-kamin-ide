//! Руби: сегменты и уровни.
mod segment_state;
mod segments;
use crate::text::ruby::segment_state::Kind;
use crate::text::ruby::segment_state::base_starts;
use crate::text::ruby::segment_state::close_segment;
use crate::text::ruby::segment_state::flush_run;
use crate::text::ruby::segment_state::fresh;
use crate::text::ruby::segments::ruby_segments;

// owner: A

use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use crate::text::text_box::blank_text;

pub(crate) mod container;
mod ruby_hiding;
pub(crate) mod ruby_transform;

/// Единица руби (css-ruby-1 §2.3.2): содержимое одной базы или одной
/// аннотации. Пустой вектор — анонимная пустая единица, добавленная спариванием.
type RubyUnit = Vec<Node>;

/// Уровень аннотаций сегмента: `<rtc>` или ряд `<rt>` прямо в контейнере
/// (анонимный контейнер аннотаций, css-ruby-1 §2.2 п.8).
struct RubyLevel {
    pub(crate) units: Vec<RubyUnit>,
    /// `<rtc>` без `<rt>` внутри — одна анонимная аннотация, накрывающая ВСЕ
    /// базы сегмента (§2.3.2 «spanning annotation»).
    pub(crate) spanning: bool,
    /// Стиль самого `<rtc>`: его аннотации наследуют от него (в том числе
    /// половинный кегль из листа агента).
    pub(crate) container: Option<Computed>,
}

/// Сегмент руби (css-ruby-1 §2.3.1): ряд баз и уровни аннотаций к нему.
struct RubySegment {
    pub(crate) bases: Vec<RubyUnit>,
    pub(crate) levels: Vec<RubyLevel>,
}

/// Пуста ли единица: только схлопываемые пробелы и руби-теги без содержимого.
/// Любой другой элемент — содержимое, даже пустой `<div>` с шириной
/// (`ruby-align-001`: `rt > div { width: 160px }`).
fn ruby_unit_blank(unit: &[Node]) -> bool {
    unit.iter().all(|n| match n {
        Node::Text(t) => blank_text(t),
        Node::Element(k)
            if ruby_role(k).is_some_and(|r| r != crate::style::computed::RubyRole::Container) =>
        {
            ruby_unit_blank(&k.children)
        }
        Node::Element(_) => false,
    })
}

/// Роль элемента в руби (css-ruby-1 §2.1): своё `display: ruby*`, иначе —
/// тег (A.1: `ruby/rb/rt/rbc/rtc`). Авторский `display` на руби-теге роль
/// СНИМАЕТ (`display: block` на `<rt>` — обычный блок, как в Blink, где
/// `IsInlineRubyText` смотрит на `Display()`, а не на тег): роль по тегу
/// действует только без своего `display`.
pub(crate) fn ruby_role(e: &Element) -> Option<crate::style::computed::RubyRole> {
    use crate::style::computed::RubyRole;
    if let Some(role) = e.style.ruby_role {
        return Some(role);
    }
    if e.style.display.is_some() {
        return None;
    }
    match e.tag.as_str() {
        "ruby" => Some(RubyRole::Container),
        "rb" => Some(RubyRole::Base),
        "rt" => Some(RubyRole::Text),
        "rbc" => Some(RubyRole::BaseContainer),
        "rtc" => Some(RubyRole::TextContainer),
        _ => None,
    }
}
