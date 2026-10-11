//! HTML list values are counter-set hints below authored counter directives.
//! CSS Lists 3 #ua-stylesheet also applies those hints to reversed-counter scans.

use super::squeeze_parens;
use crate::style::computed::{Computed, Display};

/// CSS 2 sections 6.2.1 and 12.4: explicit inheritance copies the parent's
/// computed declaration list; counter values and counter scopes are separate.
pub(crate) fn inherit_counter_decls(style: &mut Computed, parent: Option<&[Option<String>; 3]>) {
    for (i, value) in [
        &mut style.counter_reset,
        &mut style.counter_increment,
        &mut style.counter_set,
    ]
    .into_iter()
    .enumerate()
    {
        let Some(text) = value.as_deref() else {
            continue;
        };
        match text.trim().to_ascii_lowercase().as_str() {
            "inherit" => {
                *value = Some(
                    parent
                        .and_then(|p| p[i].clone())
                        .unwrap_or_else(|| "none".into()),
                )
            }
            "initial" | "unset" => *value = Some("none".into()),
            _ => {}
        }
    }
}

pub(crate) fn counter_snapshot(style: &Computed) -> [Option<String>; 3] {
    [
        style.counter_reset.clone(),
        style.counter_increment.clone(),
        style.counter_set.clone(),
    ]
}

fn list_value_hint(style: &Computed, tag: &str, attrs: &[(String, String)]) -> Option<i32> {
    if tag != "li" || style.counter_set.is_some() {
        return None;
    }
    attrs
        .iter()
        .find(|(name, _)| name == "value")
        .and_then(|(_, value)| value.trim().parse().ok())
}

pub(crate) fn apply_value_hint(style: &mut Computed, node: &crate::style::select::Ancestor) {
    if let Some(value) = list_value_hint(style, &node.tag, &node.attrs) {
        style.counter_set = Some(format!("list-item {value}"));
    }
}

/// Применить `counter-reset`/`counter-increment`/`counter-set` узла.
///
/// Порядок именно такой (css-lists-3 §5): сперва создаются счётчики, затем
/// накапливаются увеличения, затем присваиваются значения.
pub(crate) fn apply_counter_decls(
    style: &Computed,
    counters: &mut crate::style::generated::counters::Counters,
    tag: &str,
    attrs: &[(String, String)],
    item_flag: &mut bool,
    reversed_start: &dyn Fn(&str, &mut crate::style::generated::counters::Counters) -> i32,
) {
    // CSS Lists 3 §4.5: display:contents has no box, so its directives
    // do not apply, while the flattened descendants still participate.
    if style.display == Some(Display::Contents) {
        *item_flag = false;
        return;
    }
    let num_attr = |key: &str| -> Option<i32> {
        attrs
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.trim().parse().ok())
    };
    // Списочный контейнер заводит счётчик `list-item` для своих пунктов:
    // у нумерованного отсчёт начинается с `start` (css-lists-3 §ua-stylesheet
    // задаёт это правилом `ol[start] { counter-reset: list-item calc(attr(start) - 1) }`).
    // Правило таблицы агента `ol, ul, menu, dir { counter-reset: list-item }`
    // живёт в общем каскаде: авторский `counter-reset` на том же узле его
    // ЗАМЕНЯЕТ целиком, а не дополняет.
    let reversed_list = tag == "ol" && attrs.iter().any(|(k, _)| k == "reversed");
    if matches!(tag, "ol" | "ul" | "menu" | "dir") && style.counter_reset.is_none() {
        // У обратного списка отсчёт идёт вниз и начинается на единицу ВЫШЕ
        // названного, у обычного — на единицу ниже (§ua-stylesheet).
        let start = match (tag, num_attr("start")) {
            ("ol", Some(v)) if reversed_list => v + 1,
            ("ol", Some(v)) => v - 1,
            // У обратного списка без `start` отсчёт начинается с числа его
            // пунктов.
            ("ol", None) if reversed_list => reversed_start("list-item", counters),
            _ => 0,
        };
        counters.reset_flagged("list-item", start, reversed_list);
    }
    // Пункт списка увеличивает `list-item` сам, если этого не сказано явно
    // (css-lists-3 §list-item-counter). Порядок строгий: явное увеличение,
    // затем неявное, затем присваивание — иначе `<li value>` считался бы
    // от уже сдвинутого значения.
    // CSS Lists 3 §4.6: only a box with `display: list-item` increments
    // `list-item`; an `li` keeps that role only while no author display
    // replaces the UA `list-item` (`li { display: block }`).
    let is_item =
        (tag == "li" && style.display.is_none()) || style.display == Some(Display::ListItem);
    *item_flag = is_item;
    let explicit_item = style
        .counter_increment
        .as_deref()
        .is_some_and(|t| t.split_whitespace().any(|w| w == "list-item"));
    for (decl, kind) in [
        (&style.counter_reset, 0u8),
        (&style.counter_increment, 1),
        (&style.counter_set, 2),
    ] {
        if kind == 2 && is_item && !explicit_item {
            // Пункт обратного списка считает ВНИЗ (css-lists-3
            // §list-item-counter).
            let step = if counters.is_reversed("list-item") {
                -1
            } else {
                1
            };
            counters.update("list-item", step, false);
        }
        let Some(text) = decl else { continue };
        // `reversed( имя )` — одна запись, а не три слова.
        let text = squeeze_parens(text);
        let mut it = text.split_whitespace().peekable();
        while let Some(name) = it.next() {
            // `none` — ключевое слово «ничего не делать», а не имя счётчика.
            if name.eq_ignore_ascii_case("none") {
                continue;
            }
            // Обратный счётчик: имя в скобках, значение по умолчанию узнаётся
            // предварительным обходом области (пока — ноль).
            let (name, reversed) = match name
                .strip_prefix("reversed(")
                .and_then(|r| r.strip_suffix(')'))
            {
                Some(inner) if kind == 0 && !inner.is_empty() => (inner, true),
                Some(_) => continue,
                None => (name, false),
            };
            let value = match it.peek().and_then(|n| n.parse::<i32>().ok()) {
                Some(v) => {
                    it.next();
                    v
                }
                None if reversed => reversed_start(name, counters),
                None => match kind {
                    0 | 2 => 0,
                    _ => 1,
                },
            };
            match kind {
                0 => {
                    counters.reset_flagged(name, value, reversed);
                }
                1 => counters.update(name, value, false),
                _ => counters.update(name, value, true),
            }
        }
    }
    // `<li value>` задаёт номер пункта прямо (css-lists-3 §ua-stylesheet:
    // `li[value] { counter-set: list-item attr(value) }`).
    if let Some(v) = list_value_hint(style, tag, attrs) {
        counters.update("list-item", v, true);
    }
}
