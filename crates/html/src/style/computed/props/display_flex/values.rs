//! Значения длинных свойств реестра гибкой раскладки, выравнивания и
//! промежутков (css-flexbox-1 §5, §7; css-align-3 §5-§6; css-gaps-1 §2): ОДИН
//! разбор на свойство — им реестр проверяет объявление, им же обработчик
//! пишет поля стиля.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::css::LonghandId;
use crate::style::values::value::Len;

/// Годится ли значение длинному свойству гибкой раскладки, выравнивания или
/// промежутка.
pub(crate) fn accepts(id: LonghandId, value: &str, vcx: ValueCx<'_>) -> bool {
    use LonghandId::*;
    let v = value.trim();
    match id {
        FlexGrow | FlexShrink => flex_factor(v, vcx).is_some(),
        FlexBasis => flex_basis(v, vcx).is_some(),
        FlexDirection => flex_direction(v).is_some(),
        FlexWrap => flex_wrap(v).is_some(),
        AlignContent | JustifyContent | AlignItems | JustifyItems | AlignSelf | JustifySelf => {
            alignment(id, v)
        }
        RowGap | ColumnGap => gap(v, vcx).is_some(),
        _ => false,
    }
}

/// `flex-basis: content | <'width'>` (css-flexbox-1 §7.3.3): основа и признак
/// `content`. Голое ненулевое число — не длина.
pub(crate) fn flex_basis(v: &str, vcx: ValueCx<'_>) -> Option<(Len, bool)> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("content") {
        return Some((Len::Auto, true));
    }
    if crate::style::values::value::number(v, vcx).is_some_and(|n| n != 0.0) {
        return None;
    }
    size_range::parse_nonnegative_size(v, vcx).map(|len| (len, false))
}

/// `flex-direction: row | row-reverse | column | column-reverse`.
pub(crate) fn flex_direction(v: &str) -> Option<FlexDir> {
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "row" => FlexDir::Row,
        "row-reverse" => FlexDir::RowReverse,
        "column" => FlexDir::Col,
        "column-reverse" => FlexDir::ColReverse,
        _ => return None,
    })
}

/// Перенос гибких строк: (переносить, обратный порядок строк, `balance`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FlexWrap {
    pub wrap: bool,
    pub reverse: bool,
    pub balance: bool,
}

/// `flex-wrap: nowrap | [ wrap | wrap-reverse ] || balance` (css-flexbox-1
/// §5.2, css-flexbox-2 §5.2); `balance` без `wrap*` ведёт себя как `wrap`.
pub(crate) fn flex_wrap(v: &str) -> Option<FlexWrap> {
    let words: Vec<String> = v
        .split_ascii_whitespace()
        .map(str::to_ascii_lowercase)
        .collect();
    match words.as_slice() {
        [w] if w == "nowrap" => {
            return Some(FlexWrap {
                wrap: false,
                reverse: false,
                balance: false,
            });
        }
        [] => return None,
        _ => {}
    }
    let (mut mode, mut balance) = (None, false);
    for w in &words {
        match w.as_str() {
            "wrap" | "wrap-reverse" if mode.is_none() => mode = Some(w == "wrap-reverse"),
            "balance" if !balance => balance = true,
            _ => return None,
        }
    }
    Some(FlexWrap {
        wrap: true,
        reverse: mode == Some(true),
        balance,
    })
}

/// Слова значения выравнивания (без учёта регистра).
fn words(v: &str) -> Vec<String> {
    v.split_ascii_whitespace()
        .map(str::to_ascii_lowercase)
        .collect()
}

/// `<baseline-position> = [ first | last ]? baseline`.
fn baseline_position(w: &[String]) -> bool {
    match w {
        [b] => b == "baseline",
        [f, b] => matches!(f.as_str(), "first" | "last") && b == "baseline",
        _ => false,
    }
}

/// `<overflow-position>? <позиция>`: позиция — по `position`.
fn overflow_and(w: &[String], position: impl Fn(&str) -> bool) -> bool {
    match w {
        [p] => position(p),
        [o, p] => matches!(o.as_str(), "safe" | "unsafe") && position(p),
        _ => false,
    }
}

/// `<content-position>` (css-align-3 §4.3) и потоковые `flow-start`/
/// `flow-end` (WPT css-flexbox/alignment `flow-start-flow-end-*`).
fn content_position(w: &str) -> bool {
    matches!(
        w,
        "center" | "start" | "end" | "flex-start" | "flex-end" | "flow-start" | "flow-end"
    )
}

/// `<self-position>` (css-align-3 §4.2) с потоковыми словами.
fn self_position(w: &str) -> bool {
    content_position(w) || matches!(w, "self-start" | "self-end")
}

/// `<content-distribution>` (css-align-3 §4.4).
fn content_distribution(w: &[String]) -> bool {
    matches!(w, [d] if matches!(d.as_str(), "space-between" | "space-around" | "space-evenly" | "stretch"))
}

/// Годится ли значение свойству выравнивания (css-align-3 §5.1, §6.1, §6.2;
/// `anchor-center` — css-anchor-position-1 §6.1: позиция `*-self`, в том
/// числе с `safe`/`unsafe`).
pub(crate) fn alignment(id: LonghandId, v: &str) -> bool {
    use LonghandId::*;
    let w = words(v);
    let one = |k: &str| matches!(w.as_slice(), [x] if x == k);
    let lr = |p: &str| matches!(p, "left" | "right");
    match id {
        AlignContent => {
            one("normal")
                || baseline_position(&w)
                || content_distribution(&w)
                || overflow_and(&w, content_position)
        }
        JustifyContent => {
            one("normal")
                || content_distribution(&w)
                || overflow_and(&w, |p| content_position(p) || lr(p))
        }
        AlignItems => {
            one("normal")
                || one("stretch")
                || baseline_position(&w)
                || overflow_and(&w, self_position)
        }
        JustifyItems => {
            one("normal")
                || one("stretch")
                || one("legacy")
                || baseline_position(&w)
                || overflow_and(&w, |p| self_position(p) || lr(p))
                || matches!(w.as_slice(), [a, b]
                    if (a == "legacy" && matches!(b.as_str(), "left" | "right" | "center"))
                        || (b == "legacy" && matches!(a.as_str(), "left" | "right" | "center")))
        }
        AlignSelf => {
            one("auto")
                || one("normal")
                || one("stretch")
                || baseline_position(&w)
                || overflow_and(&w, |p| self_position(p) || p == "anchor-center")
        }
        JustifySelf => {
            one("auto")
                || one("normal")
                || one("stretch")
                || baseline_position(&w)
                || overflow_and(&w, |p| self_position(p) || lr(p) || p == "anchor-center")
        }
        _ => false,
    }
}

/// `row-gap`/`column-gap: normal | <length-percentage [0,∞]> | <line-width>`
/// (css-gaps-1 §2.1). `None` внутри — `normal`.
pub(crate) fn gap(v: &str, vcx: ValueCx<'_>) -> Option<Option<Len>> {
    let v = v.trim();
    match v.to_ascii_lowercase().as_str() {
        "normal" => return Some(None),
        // css-backgrounds-3 §4.3 `<line-width>`: 1px, 3px, 5px.
        "thin" => return Some(Some(Len::Px(1.0))),
        "medium" => return Some(Some(Len::Px(3.0))),
        "thick" => return Some(Some(Len::Px(5.0))),
        // Иные слова (`auto`, `stretch`, размеры по содержимому) — не длины.
        w if w.starts_with(|c: char| c.is_ascii_alphabetic()) && !w.contains('(') => return None,
        w if w.starts_with("fit-content(") => return None,
        _ => {}
    }
    if crate::style::values::value::number(v, vcx).is_some_and(|n| n != 0.0) {
        return None;
    }
    size_range::parse_nonnegative_size(v, vcx).map(Some)
}
