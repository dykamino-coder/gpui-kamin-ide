//! HTML+CSS → элементы GPUI без браузера: собственная раскладка по
//! спецификациям CSS, проверяемая reftest-ами WPT (`examples/wptrun.rs`).
//!
//! Разобрать один раз, рисовать каждый кадр:
//!
//! ```ignore
//! let doc = kamin_html::Document::builder(html, theme_css).build(); // разбор + каскад
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
pub mod coverage;
pub(crate) mod hooks;
pub mod tolerance;
pub use style::css;
pub mod dom;
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
pub(crate) mod document;
pub(crate) mod interactive;
pub(crate) mod layout;
pub(crate) mod paint;
// Сервис картинок вводится раньше потребителей: пока paint, layout и style не переведены
// на него (план I-3..I-6), часть API живёт только в тестах. Ожидание снимается само —
// компилятор потребует убрать атрибут, когда неиспользуемого кода не останется.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumers migrate to resources in I-3..I-6")
)]
pub(crate) mod resources;
pub(crate) mod style;
pub(crate) mod text;

/// Умолчания тегов, как в браузере, — для тех, кто рисует СТРАНИЦУ.
///
/// Своя таблица движка настроена под чат: там отступы мельче, а типографика
/// подчинена окну переписки. Страница же обязана выглядеть так, как её задумал
/// автор разметки, а он рассчитывал на браузерные умолчания: поле `body` в 8
/// точек, отступы абзацев и заголовков в долях кегля, отбивка списка в 40
/// точек. Передаётся вторым доводом в [`Document::builder`] — он идёт после
/// таблицы движка и до `<style>` самого документа.
// Отступы потоковых тегов — ЛОГИЧЕСКИЕ (margin-block/margin-inline), как в
// таблице HTML Rendering, Flow content: поля следуют блочной/строчной оси
// элемента, включая вертикальное письмо.
// https://html.spec.whatwg.org/multipage/rendering.html#flow-content-3
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
    /* HTML Rendering, Flow content (#flow-content-3): pre preserves whitespace
       and has block margins; it does not establish a scroll container.
       Restore initial visible overflow over the embedding chat stylesheet. */
    pre { margin-block: 1em; margin-inline: 0; padding: 0; font-size: 1em; white-space: pre;
          overflow: visible }
    code, kbd, samp { font-size: 1em }
    small { font-size: 0.83em }
    /* HTML §15.3.11: `hr { color: gray; border-style: inset; border-width: 1px;
       margin: 0.5em auto }` — the rule colour is `currentColor`, so the `color`
       hint recolours it; auto margins centre a narrower `width` hint. */
    hr { height: auto; margin: 0.5em auto; background: none; color: gray; border-style: inset;
         border-width: 1px; border-color: currentColor }
    table { margin: 0; border-spacing: 2px }
    /* HTML Rendering, Tables (#tables-2): row groups align cells in the
       middle; rows/cells inherit alignment unless an author rule overrides it. */
    thead, tbody, tfoot, table > tr { vertical-align: middle }
    /* CSS Tables 3 §10.1: the first header/footer sections repeat in print. */
    thead, tfoot { break-inside: avoid }
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
    /* UA DEVIATION: iframe lacks the 2px inset border expected by HTML
       Rendering, Embedded content (#embedded-content-rendering-rules) and
       Blink html.css. This omission has no spec basis; see
       docs/html-ua-deviations.md for the unresolved rule and acceptance case. */
    fieldset { margin-inline: 2px; border: 2px groove; padding: 0.35em 0.75em 0.625em }
    legend { padding-inline: 2px }
    a[href] { color: #0000ee }
    /* css-overflow-5: a scroll marker behaves like a link (Blink
       `scroll_marker.css`: `color: -webkit-link; text-decoration: underline`). */
    *::scroll-marker { color: #0000ee; text-decoration: underline }
    /* HTML Rendering, Button layout (#button-layout): a button generates
       an inline-block box, so its contents have an independent layout context. */
    button { display: inline-block; padding: 1px 6px; border-radius: 0 }
    /* css-overflow-5 §scroll-buttons: `::scroll-button()`s are styled like
       `<button>` (Blink `scroll_button.css`). */
    *::scroll-button(*) { display: inline-block; padding: 1px 6px; border-radius: 0;
                          box-sizing: border-box }
    /* Blink html.css gives select, checkbox and radio controls border-box
       sizing: the specified width/height includes padding and borders.
       Source: third_party/blink/renderer/core/html/resources/html.css. */
    select, input[type="checkbox" i], input[type="radio" i] { box-sizing: border-box }
    canvas { background: none; border: none }
    /* HTML Rendering, The details and summary elements
       (#the-details-and-summary-elements): the first summary is a list item
       with a disclosure marker reflecting the details open state. */
    details > summary:first-of-type { display: list-item; counter-increment: list-item 0;
                                      list-style: disclosure-closed inside }
    details[open] > summary:first-of-type { list-style-type: disclosure-open }
    mark { background: yellow; color: black }
    textarea { background: Field; color: FieldText; white-space: pre-wrap;
               overflow-wrap: break-word; border: 1px solid #767676;
               border-radius: 0; padding: 2px; margin: 0 }
"#;

pub use document::{Document, DocumentBuilder};
pub use dom::{Element, Node, parse};
pub use render::{RenderOpts, render, render_block};
