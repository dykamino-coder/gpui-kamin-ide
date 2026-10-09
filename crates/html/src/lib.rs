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

// Стилевые lint'ы clippy, которые в движке вёрстки дают только шум: длинные
// сигнатуры и типы отражают CSS-модель, индексные циклы идут по нескольким
// массивам сразу, а `Default` + присваивания читаются как таблица свойств.
// Корректностные lint'ы остаются включёнными и держатся под `-D warnings`.
#![allow(
    clippy::type_complexity,
    clippy::field_reassign_with_default,
    clippy::too_many_arguments,
    clippy::large_enum_variant,
    clippy::needless_range_loop,
    clippy::manual_clamp,
    clippy::nonminimal_bool,
    clippy::redundant_guards
)]

pub mod anchor;
pub mod apply;
pub mod background;
pub mod band_flow;
pub mod bands;
pub mod border_image;
pub mod color_space;
pub mod computed;
pub mod counter_style;
pub mod counters;
pub mod counters_scan;
pub mod coverage;
pub mod css;
pub mod doc;
pub mod dom;
pub mod encoding;
pub mod float;
pub mod flow;
pub mod fonts;
pub mod forms;
pub mod inline;
pub mod interact;
pub mod lines;
pub mod metrics;
pub mod page_margin;
mod motion;
pub mod render;
pub mod scroll;
pub mod select;
pub mod svg;
pub mod transition;
pub mod value;
pub mod zoom;

/// Умолчания тегов, как в браузере, — для тех, кто рисует СТРАНИЦУ.
///
/// Своя таблица движка настроена под чат: там отступы мельче, а типографика
/// подчинена окну переписки. Страница же обязана выглядеть так, как её задумал
/// автор разметки, а он рассчитывал на браузерные умолчания: поле `body` в 8
/// точек, отступы абзацев и заголовков в долях кегля, отбивка списка в 40
/// точек. Передаётся вторым доводом в [`Document::new`] — он идёт после
/// таблицы движка и до `<style>` самого документа.
// Отступы потоковых тегов — ЛОГИЧЕСКИЕ (margin-block/margin-inline), как в
// настоящей таблице UA (whatwg rendering §15.3.3): при вертикальном письме
// они поворачиваются на горизонтальную ось (wm-propagation-body-*).
pub const BROWSER_CSS: &str = r#"
    body { margin: 8px }
    p { margin-block: 1em; margin-inline: 0 }
    h1, h2, h3, h4, h5, h6 { font-weight: bold }
    h1 { font-size: 2em; margin-block: 0.67em; margin-inline: 0 }
    h2 { font-size: 1.5em; margin-block: 0.83em; margin-inline: 0 }
    h3 { font-size: 1.17em; margin-block: 1em; margin-inline: 0 }
    h4 { font-size: 1em; margin-block: 1.33em; margin-inline: 0 }
    h5 { font-size: 0.83em; margin-block: 1.67em; margin-inline: 0 }
    h6 { font-size: 0.67em; margin-block: 2.33em; margin-inline: 0 }
    ul, ol { margin-block: 1em; margin-inline: 0; padding-inline-start: 40px }
    ol { list-style-type: decimal }
    ul, menu, dir { list-style-type: disc }
    li { margin: 0 }
    dl { margin-block: 1em; margin-inline: 0 }
    dd { margin-left: 40px }
    dt { font-weight: normal; margin: 0 }
    blockquote { margin-block: 1em; margin-inline: 40px; padding-left: 0; border-left: none }
    figure { margin-block: 1em; margin-inline: 40px }
    figcaption { font-size: 1em; margin: 0 }
    /* `overflow-x: auto` чатового листа браузер у `pre` не ставит: прокручиваемая
       коробка теряла прижим к концу в rtl-родителе (`text-align-start-014`). */
    pre { margin-block: 1em; margin-inline: 0; padding: 0; font-size: 1em; white-space: pre;
          overflow: visible }
    code, kbd, samp { font-size: 1em }
    small { font-size: 0.83em }
    hr { height: 0; margin: 0.5em 0; background: none; border: 1px inset gray }
    table { margin: 0; border-spacing: 2px }
    /* HTML §15.3.9: row groups center cells unless authored alignment wins. */
    thead, tbody, tfoot, table > tr { vertical-align: middle }
    tr, td, th { vertical-align: inherit }
    th { padding: 1px; font-weight: bold; text-align: center }
    td { padding: 1px }
    caption { font-weight: normal; margin: 0; text-align: center }
    center { text-align: center }
    address, cite, dfn, var { font-style: italic }
    big { font-size: larger }
    sub { vertical-align: sub; font-size: smaller }
    sup { vertical-align: super; font-size: smaller }
    u, ins { text-decoration: underline }
    s, strike, del { text-decoration: line-through }
    nobr { white-space: nowrap }
    /* ПРОБОВАЛИ И ОТКАТИЛИ: `iframe { border: 2px inset }` из листа HTML.
       Замерено: CSS3 2352 -> 2350, потеряны position-absolute-iframe-print-001
       и -002 (0.23 -> 1.39); эталоны этих пар рамку рамке не рисуют. */
    fieldset { margin-inline: 2px; border: 2px groove; padding: 0.35em 0.75em 0.625em }
    legend { padding-inline: 2px }
    a[href] { color: #0000ee }
    /* `button` — строчный блок (HTML §15.5.2 «expected to render as an
       'inline-block' box»): без вида кнопка шла строчной коробкой, и доля
       высоты картинки внутри не решалась (`intrinsic-percent-replaced-021`). */
    button { display: inline-block; padding: 1px 6px; border-radius: 0 }
    canvas { background: none; border: none }
    mark { background: yellow; color: black }
    textarea { background: white; color: black; white-space: pre-wrap;
               overflow-wrap: break-word; border: 1px solid #767676;
               border-radius: 0; padding: 2px; margin: 0 }
"#;

pub use doc::Document;
pub use dom::{Element, Node, parse};
pub use render::{RenderOpts, render, render_block};
