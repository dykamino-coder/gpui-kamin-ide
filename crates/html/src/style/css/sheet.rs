//! Таблица стилей: правила, каскадные слои, пространства имён.

use crate::document::state::ValueCx;
use crate::style::css::*;

pub(crate) mod default_ns;
mod nesting;
mod parse_cx;
pub use parse_cx::LayerRegistry;
pub(crate) use parse_cx::{CustomMedia, SelectorCx, SheetCx};
mod rules;
use rules::sheet_rules;

/// Normalize nesting selectors through the same path as top-level style rules.
pub(super) fn selector_for_supports(selector: &str, cx: &SelectorCx) -> String {
    nesting::top_level_parts(&[selector], cx).remove(0)
}

pub fn parse_stylesheet(css: &str) -> Result<Vec<Rule>, crate::dom::outcome::DocumentFailure> {
    parse_stylesheet_media(css, Media::default())
}

/// Префиксы действительных `@namespace` (css-namespaces-3 §2): «must follow
/// all @charset and @import rules and precede all other non-ignored at-rules
/// and style rules … Otherwise the @namespace rule is invalid». Даже пустой
/// `@media {}` или `@supports (…) {}` закрывает пролог (`at-media-003`,
/// `at-supports-045`).
/// Возвращает префиксы и пространство по умолчанию (`url(…)` без префикса).
fn declared_prefixes(css: &str) -> (std::collections::HashSet<String>, Option<bool>) {
    let mut set = std::collections::HashSet::new();
    let mut default_ns = None;
    let cleaned = strip_comments(css);
    let mut rest = cleaned.as_str();
    loop {
        rest = stylesheet_tokens::start(rest);
        let Some((piece, tail)) = next_piece(rest) else {
            break;
        };
        rest = tail;
        let Piece::Statement { head } = piece else {
            break;
        };
        let low = head.trim().to_ascii_lowercase();
        if low.starts_with("@charset") || low.starts_with("@import") || low.starts_with("@layer") {
            continue;
        }
        let Some(r) = low.strip_prefix("@namespace") else {
            break;
        };
        let first = r.split_whitespace().next().unwrap_or("");
        if first.starts_with('"') || first.starts_with(char::from(39)) || first.starts_with("url(")
        {
            default_ns = Some(first.contains("1999/xhtml"));
        }
        if !first.is_empty()
            && !first.starts_with('"')
            && !first.starts_with('\'')
            && !first.starts_with("url(")
        {
            set.insert(first.to_string());
        }
    }
    (set, default_ns)
}

/// `attr(ns|name)` с необъявленным префиксом делает объявление негодным —
/// для оракула `@supports` (`at-supports-namespace-001`: `attr(y|href)`).
pub(super) fn attr_prefixes_declared(v: &str, cx: &SelectorCx) -> bool {
    let low = v.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(at) = low[from..].find("attr(") {
        let open = from + at + 5;
        let arg = low[open..]
            .trim_start()
            .split(|ch: char| ch == ')' || ch == ',' || ch.is_whitespace())
            .next()
            .unwrap_or("");
        if let Some((ns, _)) = arg.split_once('|')
            && !cx.ns_declared(ns)
        {
            return false;
        }
        from = open;
    }
    true
}

/// То же, но с известными условиями окружения. Таблица разбирается сама по
/// себе — со своим реестром слоёв и своим документом для at-правил; таблицы
/// документа делят его (`parse_document_sheet`).
pub fn parse_stylesheet_media(
    css: &str,
    media: Media,
) -> Result<Vec<Rule>, crate::dom::outcome::DocumentFailure> {
    let budget = crate::budget::DocumentBudget::new(Default::default(), Default::default())
        .map_err(|error| {
            crate::dom::outcome::DocumentFailure::new(crate::dom::outcome::FailureReason::Budget(
                error,
            ))
        })?;
    let revision = budget.revision();
    revision.source("", css).map_err(|error| {
        crate::dom::outcome::DocumentFailure::new(crate::dom::outcome::FailureReason::Budget(error))
    })?;
    crate::css_syntax::admission::admit(
        css,
        &crate::budget::admission::Admission::new(revision.clone()),
    )?;
    let doc = crate::document::state::DocState::new(media);
    doc.budget.replace(Some(revision));
    Ok(parse_document_sheet(
        css,
        media,
        &mut LayerRegistry::default(),
        doc.values(),
    ))
}

/// Таблица документа: слои — в общем реестре `layers`, пролог `@namespace`
/// и `@custom-media` — свои у каждой таблицы. Вложенные группы (`@media`,
/// `@supports`, `@layer`) разбираются тем же контекстом.
pub(crate) fn parse_document_sheet(
    css: &str,
    media: Media,
    layers: &mut LayerRegistry,
    vcx: ValueCx<'_>,
) -> Vec<Rule> {
    let (prefixes, default_ns) = declared_prefixes(css);
    let mut cx = SheetCx {
        doc: vcx.doc,
        layers,
        layer_now: (String::new(), Vec::new()),
        selectors: SelectorCx {
            prefixes: Some(std::rc::Rc::new(prefixes)),
            default_ns,
        },
        custom_media: custom_media_of(css),
    };
    sheet_rules(css, media, true, &mut cx)
}
