//! HTML+CSS → элементы GPUI без браузера: собственная раскладка по
//! спецификациям CSS, проверяемая reftest-ами WPT (`examples/wptrun.rs`).
//!
//! Разобрать один раз, рисовать каждый кадр:
//!
//! ```ignore
//! let doc = kamin_html::Document::builder(html, theme_css).build()?; // разбор + каскад
//! let elements = doc.render(&RenderOpts { text, ..Default::default() });
//! ```
//!
//! Конвейер — папки по фазам: `dom/` (html5ever → дерево узлов, обход с
//! каскадом, починка дерева коробок) → `style/` (разбор CSS, селекторы,
//! каскад, `Computed`) → `layout/` и `text/` (блочный и строчный поток,
//! флоаты, таблицы, многоколоночник, фрагментация, страницы,
//! позиционирование, абзац, руби, вертикальное письмо) → `paint/` и `svg/`
//! (перевод `Computed` в стиль элемента GPUI, фон, рамки, эффекты,
//! наложение; `<svg>` через resvg). `render/` — сборка: обходит узлы и
//! зовёт фазы, получая готовые элементы GPUI. `document/` держит разобранный
//! документ между кадрами, `interactive/` — прокрутку, липкие коробки, формы
//! и выделение, `animation/` — переходы, анимации и `offset-path`. Границы
//! и направление зависимостей — `ARCHITECTURE.md`, раздел «Движок HTML/CSS».
//!
//! Что покрыто, что нет и почему — реестр `coverage.rs`: каждое свойство
//! классифицировано, тест проверяет, что помеченное `Mapped` меняет
//! разрешённый стиль. Таблица в `docs/html-css-mapping.html` собирается из
//! реестра скриптом `scripts/css_coverage_table.py`; пояснительные разделы
//! той страницы старше текущей раскладки и описывают историю решений.
//! Сознательно вне цели: выполнение скриптов (JS) и всё, что требует его.

#![forbid(unsafe_code)]

#[cfg(test)]
mod depth_tests;

#[cfg(test)]
pub mod coverage;
pub(crate) mod defaults;
pub(crate) mod hooks;
pub mod tolerance;
pub use style::css;
pub(crate) mod box_facts;
pub mod dom;
pub mod font_facts;
pub use dom::encoding;
/// Внешний путь геометрии страниц (`kamin_html::flow::PageGeom`).
pub mod flow {
    pub use crate::layout::page::page_stack::{PageGeom, PageGeomFn};
}
pub use text::fonts;
pub use text::metrics;
pub mod render;
pub use interactive::scroll;
pub use interactive::select;
pub mod svg;
pub use style::values::value;

// Доменные папки конвейера закрыты (`pub(crate)`): их типы — внутренний
// контракт между фазами, а не API крейта. Наружу выходит только объявленное
// `pub` в этом файле: `dom`, `render`, `svg`, `tolerance`, `flow`, реэкспорты.
pub(crate) mod animation;
pub(crate) mod cache;
pub(crate) mod color;
pub(crate) mod css_syntax;
pub(crate) mod document;
pub(crate) mod interactive;
pub(crate) mod layout;
pub(crate) mod paint;
pub(crate) mod resources;
pub(crate) mod style;
pub(crate) mod text;

pub use style::ua::BROWSER_CSS;

#[cfg(feature = "wpt-harness")]
pub use layout::fragment::probe::diagnostics::{FragmentProbe, FragmentProbes};

pub use document::{
    BuildReport, Document, DocumentBuilder, DocumentFailure, FailureReason, FailureScope,
    RevisionStatus, TraversalKind, TraversalLimit,
};
pub use dom::{Element, Node, try_parse as parse};
pub use render::{RenderOpts, render, render_block};
