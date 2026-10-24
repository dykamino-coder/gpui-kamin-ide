//! HTML → дерево узлов с вычисленным стилем.
//!
//! Разбор отдан `html5ever` — тому же парсеру, что стоит в браузерах на Rust:
//! писать свой означало бы повторять правила восстановления после ошибок
//! (незакрытые теги, неявные `<tbody>`), которые модель нарушает регулярно.
//! Наша часть — превратить его дерево в своё: с каскадом и без узлов, которые
//! ничего не рисуют.

mod anonymous;
mod direction;
mod shared_style;
pub use shared_style::SharedStyle;
pub(crate) mod input;
pub(crate) mod mathml;
mod mathml_defaults;
mod mathml_lengths;
mod mathml_routing;
#[cfg(test)]
mod nth_child_tests;
mod page_scheme;
mod parse_document;
pub(crate) mod pseudo_line_layers;
pub(crate) mod style_sources;
pub(crate) mod text_content;
pub(crate) mod tree_index;
#[cfg(test)]
mod visited_tests;
pub(crate) mod xml;
pub(crate) use xml::parse_current_xml;
pub use xml::parse_xml;
#[cfg(test)]
mod presentational_hint_tests;
#[cfg(test)]
mod presentational_tests;
mod quirks_color;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
mod ua_style;
#[cfg(test)]
mod white_space_tests;
use crate::dom::direction::first_strong;
use crate::dom::direction::squeeze_parens;
pub use crate::dom::parse_document::parse;
pub(crate) use crate::dom::parse_document::parse_current;
pub use crate::dom::parse_document::parse_media;
mod parsed;
use crate::dom::style_sources::collect_style_tags;
use crate::dom::ua_style::BLOCK_TAGS;
use crate::dom::ua_style::user_agent_css;
pub use parsed::Parsed;

mod containment;
mod counter_decls;
mod display_inheritance;
pub mod encoding;
mod float_tail;
mod grid_static_position;
mod initial_pseudos;
pub(super) mod language;
mod presentational_hints;
mod presentational_hr;
mod presentational_legacy;
mod replaced_display;
pub(crate) mod resource_facts;
mod subgrid_axes;
mod subgrid_used;
pub(crate) mod table_boxes;
pub(crate) mod whitespace;
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
mod marks;
pub(super) mod pseudo;
mod state_identity;
pub use crate::dom::marks::Marks;
pub(crate) use crate::dom::marks::{InlineCb, LeadProbe, ShapeFlow};
pub(crate) use crate::dom::pseudo::*;

/// Узел документа: либо текст, либо элемент со своими детьми.
#[derive(Clone, Debug)]
// Keep tree metadata inline; computed styles already live behind pointers.
#[allow(clippy::large_enum_variant)]
pub enum Node {
    Text(String),
    Element(Element),
}

#[derive(Clone, Debug)]
pub struct Element {
    /// Номер пункта списка из счётчика `list-item`; `None` — не пункт.
    pub list_item: Option<i64>,
    /// Устойчивый номер авторского узла; ноль обозначает синтетическую коробку.
    /// Идентичность состояния GPUI для неё хранится отдельно в Marks.
    pub node_id: u64,
    /// Кадры анимации, уже разрешённые в стиль: доля времени → стиль.
    pub anim: Option<std::rc::Rc<Vec<(f32, Computed)>>>,
    pub tag: String,
    pub style: SharedStyle,
    /// Стиль наведения, собранный из правил с `:hover`. Пустой, если таких
    /// правил не было.
    pub hover: Option<SharedStyle>,
    /// Стиль первой буквы абзаца (`::first-letter`) — отдельным слоем поверх
    /// базового: это буквица, а не стиль всего блока.
    pub first_letter: Option<SharedStyle>,
    /// Стиль первой строки абзаца (`::first-line`).
    pub first_line: Option<SharedStyle>,
    pub children: Vec<Node>,
    /// Атрибуты, которые нужны при отрисовке: `src`, `href`, `colspan`.
    pub attrs: Vec<(String, String)>,
    /// Инлайн ли элемент по своей природе (`<span>`, `<code>`, `<a>`): от
    /// этого зависит, попадёт ли он в строку текста или станет блоком.
    pub inline: bool,
    /// Внутренние layout-метки и идентичность коробки; не задаются разметкой.
    pub marks: Marks,
}

impl Element {
    /// A box layout inserts into the tree (anonymous block, float host, row):
    /// no markup node, attributes or pseudo-element layers of its own.
    pub(crate) fn synthetic(tag: &str, style: Computed, marks: Marks, children: Vec<Node>) -> Self {
        Element {
            list_item: None,
            node_id: 0,
            anim: None,
            tag: tag.into(),
            style: style.into(),
            hover: None,
            first_letter: None,
            first_line: None,
            children,
            attrs: Vec::new(),
            inline: false,
            marks,
        }
    }

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
