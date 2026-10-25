//! Таблица стилей агента: умолчания тегов и список блочных тегов.
//!
//! `BLOCK_TAGS` — теги, которым лист агента даёт блочный вид (HTML
//! §15.3.2–15.3.12); неизвестный тег — строчный. `user_agent_css` — лист
//! умолчаний (заголовки, списки, код, руби по css-ruby-1 A.1, кавычки `q`,
//! `marquee`), идущий первым происхождением каскада; `QUIRKS_CSS` — добавка
//! режима quirks (цвет таблиц, маркеры сирот-пунктов). Лист чатовый: стенд
//! подаёт браузерные значения вторым листом того же происхождения.

/// Теги, которым таблица агента даёт блочный вид (HTML §15.3.2-15.3.12).
///
/// Правило для НЕИЗВЕСТНОГО тега — строчный: своей записи в листе агента у
/// него нет, а начальное значение `display` — `inline`. Прежде блочным
/// становилось всё, чего нет в `INLINE_TAGS`, и `<foo>` внутри абзаца рвал
/// строку (`line-breaking-font-size-zero-001`).
pub(super) const BLOCK_TAGS: &[&str] = &[
    "html",
    "body",
    "address",
    "blockquote",
    "center",
    "div",
    "figure",
    "figcaption",
    "footer",
    "form",
    "header",
    "hr",
    "legend",
    "listing",
    "main",
    "p",
    "plaintext",
    "pre",
    "xmp",
    "article",
    "aside",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hgroup",
    "nav",
    "section",
    "search",
    "dir",
    "dd",
    "dl",
    "dt",
    "ol",
    "ul",
    "menu",
    "li",
    "table",
    "caption",
    "colgroup",
    "col",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "td",
    "th",
    "fieldset",
    "details",
    "summary",
    "dialog",
    "optgroup",
    "option",
    "frameset",
    "frame",
    "noframes",
    "head",
    "title",
    "meta",
    "link",
    "base",
    "script",
    "style",
    "noscript",
    "template",
    "slot",
    "map",
    "area",
    "source",
    "track",
    "param",
];

pub(super) use crate::style::ua::{QUIRKS_CSS, user_agent_css};
