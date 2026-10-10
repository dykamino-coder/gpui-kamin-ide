//! Direction for ruby_display; split out to keep the owning module within 250 lines.

use crate::dom::*;
use crate::style::computed::{Computed, Display, Position};
use crate::style::select::Ancestor;

/// Направление письма, заданное АТРИБУТОМ: `<div dir="rtl">`.
///
/// В разметке направление задают именно атрибутом, а не стилем: он и есть
/// обычный способ написать страницу справа налево. Тег `<bdo>` вдобавок
/// ОТМЕНЯЕТ разбор двунаправленности — знаки идут ровно в заданную сторону.
pub(crate) fn apply_direction(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
    let Some((_, value)) = attrs.iter().find(|(k, _)| k == "dir") else {
        if tag == "bdo" {
            style.bidi_override = Some(true);
        }
        return;
    };
    // Атрибут `dir` (и `auto`: сторону потом решает первый сильный знак,
    // `render.rs`) — встраивание (прежний ход: RLE/LRE … PDF), пока стиль
    // не задал `unicode-bidi` сам. HTML UA-лист даёт `[dir] { unicode-bidi:
    // isolate }`; здесь сохранено прежнее встраивание — переход на
    // изоляцию отдельный шаг с замером.
    // HTML §15.3.4 (Bidirectional text): `[dir=ltr i], [dir=rtl i] {
    // unicode-bidi: isolate }` — изоляция, а не встраивание: строчный
    // `<span dir=rtl>` в rtl-абзаце не перемешивается с соседним ltr-текстом
    // (`text-overflow-string-007/008-ref`). `auto` пока остаётся прежним
    // встраиванием: сторону ему выбирает отрисовка.
    let explicit = style.bidi_embed.is_some() || style.bidi_isolate.is_some();
    match value.to_ascii_lowercase().as_str() {
        "rtl" | "ltr" if !explicit && tag != "bdo" => style.bidi_isolate = Some(true),
        "rtl" | "ltr" | "auto" if style.bidi_embed.is_none() => style.bidi_embed = Some(true),
        _ => {}
    }
    match value.to_ascii_lowercase().as_str() {
        "rtl" => {
            if style.rtl.is_none() {
                style.rtl = Some(true)
            }
        }
        "ltr" if style.rtl.is_none() => style.rtl = Some(false),
        // `dir="auto"` — сторону выбирает первый сильный знак текста; это
        // делает разбор двунаправленности сам, поэтому здесь ничего не ставим.
        _ => {}
    }
    if tag == "bdo" {
        style.bidi_override = Some(true);
    }
}

/// css-ruby-1 §2.2 п.1 «Inlinify block-level boxes».
///
/// Коробка блочного УРОВНЯ в потоке, лежащая внутри руби-коробки (контейнер,
/// `rb`, `rt`, `rbc`, `rtc`), получает строчный аналог: `block`/`list-item`
/// -> `inline-block`, `table` -> `inline-table`, `flex` -> `inline-flex`,
/// `grid` -> `inline-grid`; блочный ПО ТЕГУ элемент без своего `display`
/// (`<div>`, `<p>`, `<li>`) — `inline-block`. Внутренние табличные виды не
/// трогаются: их по спеке заворачивает АНОНИМНАЯ строчная таблица, которой у
/// нас нет (`ruby-inlinize-blocks-003` этим рукавом не берётся). Правило
/// проходит сквозь неатомарные строчные звенья (`<b>` внутри `<ruby>`), но не
/// сквозь блок или атом: те начинают свой поток.
///
/// `ancestors` — цепочка предков от БЛИЖАЙШЕГО. Руби узнаётся по имени тега:
/// `display: ruby*` пока не разбирается, а разметка набора пишется тегами.
pub(crate) fn inlinify_in_ruby<'a>(
    style: &mut Computed,
    tag: &str,
    ancestors: impl Iterator<Item = &'a Ancestor>,
) {
    // Вне потока коробка блокифицируется (§9.7) и инлайнизации не подлежит.
    if style.float.is_some_and(|f| f != 0)
        || matches!(
            style.position,
            Some(Position::Absolute) | Some(Position::Fixed)
        )
    {
        return;
    }
    let mut inside = false;
    for a in ancestors {
        if matches!(a.tag.as_str(), "ruby" | "rb" | "rt" | "rbc" | "rtc") {
            inside = true;
            break;
        }
        if !INLINE_TAGS.contains(&a.tag.as_str()) {
            break;
        }
    }
    if !inside {
        return;
    }
    style.display = match style.display {
        Some(Display::Block) | Some(Display::ListItem) => Some(Display::InlineBlock),
        Some(Display::Table) => Some(Display::InlineTable),
        Some(Display::Flex) => Some(Display::InlineFlex),
        Some(Display::Grid) => Some(Display::InlineGrid),
        None if tag == "table" => Some(Display::InlineTable),
        None if BLOCK_TAGS.contains(&tag) => Some(Display::InlineBlock),
        other => other,
    };
}
