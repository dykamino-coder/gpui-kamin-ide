//! HTML → дерево узлов с вычисленным стилем.
//!
//! Разбор отдан `html5ever` — тому же парсеру, что стоит в браузерах на Rust:
//! писать свой означало бы повторять правила восстановления после ошибок
//! (незакрытые теги, неявные `<tbody>`), которые модель нарушает регулярно.
//! Наша часть — превратить его дерево в своё: с каскадом и без узлов, которые
//! ничего не рисуют.

mod direction;
#[cfg(test)]
mod nth_child_tests;
mod parse_document;
#[cfg(test)]
mod presentational_tests;
#[cfg(test)]
mod tests;
mod ua_style;
#[cfg(test)]
mod white_space_tests;
use crate::dom::direction::first_strong;
use crate::dom::direction::squeeze_parens;
use crate::dom::parse_document::collect_style_tags;
pub use crate::dom::parse_document::parse;
pub use crate::dom::parse_document::parse_media;
use crate::dom::ua_style::BLOCK_TAGS;
use crate::dom::ua_style::user_agent_css;

mod containment;
mod counter_decls;
mod display_inheritance;
pub mod encoding;
mod float_tail;
mod grid_static_position;
mod initial_pseudos;
pub(super) mod language;
mod presentational_hints;
mod replaced_display;
mod subgrid_axes;
pub(crate) use crate::dom::counter_decls::{
    apply_counter_decls, apply_value_hint, counter_snapshot, inherit_counter_decls,
};
pub(super) mod content;
pub(crate) use crate::dom::content::{content_text, host_content, resolve_content_attributes};

use crate::style::computed::Computed;
pub(super) mod fixup_tree;
pub(super) mod xhtml;
use crate::dom::fixup_tree::*;
pub(super) mod fixup_grid;
pub(crate) use crate::dom::fixup_grid::*;
pub(crate) mod shadow;
pub(crate) use crate::dom::shadow::*;
pub(super) mod element_style;
use crate::dom::element_style::*;
pub(super) mod scroll_markers;
pub(super) mod walk;
use crate::dom::scroll_markers::*;
pub(super) mod pseudo;
pub(crate) use crate::dom::pseudo::*;

/// Узел документа: либо текст, либо элемент со своими детьми.
#[derive(Clone, Debug)]
pub enum Node {
    Text(String),
    Element(Element),
}

#[derive(Clone, Debug)]
pub struct Element {
    /// Номер пункта списка из счётчика `list-item`; `None` — не пункт.
    pub list_item: Option<i32>,
    /// Устойчивый номер узла в документе.
    ///
    /// Нужен анимации: GPUI хранит её состояние по идентификатору элемента, а
    /// он обязан совпадать от кадра к кадру, иначе анимация каждый раз
    /// начинается заново.
    pub node_id: u64,
    /// Кадры анимации, уже разрешённые в стиль: доля времени → стиль.
    pub anim: Option<Vec<(f32, Computed)>>,
    pub tag: String,
    pub style: Computed,
    /// Стиль наведения, собранный из правил с `:hover`. Пустой, если таких
    /// правил не было.
    pub hover: Option<Computed>,
    /// Стиль первой буквы абзаца (`::first-letter`) — отдельным слоем поверх
    /// базового: это буквица, а не стиль всего блока.
    pub first_letter: Option<Computed>,
    /// Стиль первой строки абзаца (`::first-line`).
    pub first_line: Option<Computed>,
    pub children: Vec<Node>,
    /// Атрибуты, которые нужны при отрисовке: `src`, `href`, `colspan`.
    pub attrs: Vec<(String, String)>,
    /// Инлайн ли элемент по своей природе (`<span>`, `<code>`, `<a>`): от
    /// этого зависит, попадёт ли он в строку текста или станет блоком.
    pub inline: bool,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Теги, которые в HTML участвуют в строке текста, а не разрывают её.
pub(super) const INLINE_TAGS: &[&str] = &[
    "a", "abbr", "b", "bdi", "bdo", "br", "cite", "code", "data", "dfn", "em", "i", "kbd", "mark",
    // Все руби-теги — строчные (css-ruby-1 §2.1.1: контейнер и внутренние
    // коробки руби неатомарны и строчного уровня).
    "q", "rb", "rbc", "rp", "rt", "rtc", "ruby", "s", "samp", "small", "span", "strong", "sub",
    "sup", "time", "u", "var", "wbr", "img", "svg",
    // Правки текста: без них `~~зачёркнутое~~` из markdown разрывало абзац.
    "del", "ins",
    // Управление формой стоит В СТРОКЕ: иначе «Согласен» уезжает под флажок,
    // а ряд кнопок выстраивается в столбик.
    "button", "label", "input", "select", "textarea", "output", "meter", "progress",
];

/// Теги, содержимое которых не рисуется НИКОГДА (код и стили).
///
/// `head`/`title`/`meta`/`link` сюда не входят: их прячет таблица агента
/// `display: none`, и авторское `head { display: block }` её перебивает
/// (CSS2/generated-content content-067 и родня).
pub(super) const DROP_TAGS: &[&str] = &["script", "style", "noscript"];

/// Имя тега без пространственного префикса.
///
/// В XHTML рисунок и формулы часто пишут с префиксом (`<svg:svg
/// xmlns:svg="…">`), а разборщик HTML держит двоеточие частью имени. Движок
/// сверяет теги по коротким именам, поэтому `svg:svg` не опознавался как
/// рисунок вовсе — вся семья замещаемых тестов CSS2 рисовала пустоту.
pub(super) fn local_name(name: &str) -> String {
    match name.split_once(':') {
        Some((_, local)) if !local.is_empty() => local.to_string(),
        _ => name.to_string(),
    }
}

// Кастомные свойства из правил. Селектор не важен: в документе переменные
// почти всегда объявлены на корне, а разбирать их область видимости — это
