//! HTML+CSS → элементы GPUI.
//!
//! ```ignore
//! let nodes = kamin_html::parse(html, theme_css);
//! let elements = kamin_html::render(&nodes, &RenderOpts { text, ..Default::default() });
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
pub mod background;
pub mod border_image;
pub mod color_space;
pub mod computed;
pub mod coverage;
pub mod css;
pub mod doc;
pub mod dom;
pub mod encoding;
pub mod float;
pub mod fonts;
pub mod forms;
pub mod inline;
pub mod interact;
pub mod lines;
pub mod metrics;
pub mod render;
pub mod scroll;
pub mod select;
pub mod svg;
pub mod transition;
pub mod value;

/// Умолчания тегов, как в браузере, — для тех, кто рисует СТРАНИЦУ.
///
/// Своя таблица движка настроена под чат: там отступы мельче, а типографика
/// подчинена окну переписки. Страница же обязана выглядеть так, как её задумал
/// автор разметки, а он рассчитывал на браузерные умолчания: поле `body` в 8
/// точек, отступы абзацев и заголовков в долях кегля, отбивка списка в 40
/// точек. Передаётся вторым доводом в [`Document::new`] — он идёт после
/// таблицы движка и до `<style>` самого документа.
pub const BROWSER_CSS: &str = r#"
    body { margin: 8px }
    p { margin: 1em 0 }
    h1, h2, h3, h4, h5, h6 { font-weight: bold }
    h1 { font-size: 2em; margin: 0.67em 0 }
    h2 { font-size: 1.5em; margin: 0.83em 0 }
    h3 { font-size: 1.17em; margin: 1em 0 }
    h4 { font-size: 1em; margin: 1.33em 0 }
    h5 { font-size: 0.83em; margin: 1.67em 0 }
    h6 { font-size: 0.67em; margin: 2.33em 0 }
    ul, ol { margin: 1em 0; padding-left: 40px }
    li { margin: 0 }
    dl { margin: 1em 0 }
    dd { margin-left: 40px }
    dt { font-weight: normal; margin: 0 }
    blockquote { margin: 1em 40px; padding-left: 0; border-left: none }
    figure { margin: 1em 40px }
    figcaption { font-size: 1em; margin: 0 }
    pre { margin: 1em 0; padding: 0; font-size: 1em; white-space: pre }
    code, kbd, samp { font-size: 1em }
    small { font-size: 0.83em }
    hr { height: 0; margin: 0.5em 0; background: none; border: 1px inset gray }
    table { margin: 0; border-spacing: 2px }
    th { padding: 1px; font-weight: bold; text-align: center }
    td { padding: 1px }
    caption { font-weight: normal; margin: 0 }
    a { color: #0000ee }
    button { padding: 1px 6px; border-radius: 0 }
    canvas { background: none; border: none }
    mark { background: yellow; color: black }
    textarea { background: white; color: black; white-space: pre-wrap;
               overflow-wrap: break-word; border: 1px solid #767676;
               border-radius: 0; padding: 2px; margin: 0 }
"#;

pub use doc::Document;
pub use dom::{Element, Node, parse};
pub use render::{RenderOpts, render, render_block};
