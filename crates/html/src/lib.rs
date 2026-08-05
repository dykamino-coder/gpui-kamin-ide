//! HTML+CSS → элементы GPUI.
//!
//! ```ignore
//! let nodes = kamin_html::parse(html, theme_css);
//! let elements = kamin_html::render(&nodes, &RenderOpts { text, table_min_col: 80. });
//! ```
//!
//! Что покрыто, что нет и почему — `docs/html-css-mapping.html`; там же по
//! каждому свойству пример и объяснение. Коротко: бокс-модель, флекс, текст,
//! цвета и фон, рамки со скруглением, внешние тени, простой grid, списки,
//! таблицы, картинки — переносятся; инлайн-поток — с оговоркой (см.
//! `inline.rs`); трансформы, фильтры, обрезка по произвольному контуру,
//! `z-index`, переходы и анимации — не переносятся, потому что примитивов под
//! них в GPUI нет.

pub mod apply;
pub mod computed;
pub mod css;
pub mod doc;
pub mod dom;
pub mod inline;
pub mod render;
pub mod svg;
pub mod value;

pub use doc::Document;
pub use dom::{Element, Node, parse};
pub use render::{RenderOpts, render, render_block};
