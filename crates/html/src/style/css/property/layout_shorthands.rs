//! Сокращения раскладки, раскрытые при разборе: `flex` (css-flexbox-1 §7.1),
//! `flex-flow` (§5.3), `gap` (css-gaps-1 §2.2), `place-content`/`-items`/
//! `-self` (css-align-3 §5.3, §6.3, §6.4) и `columns` (css-multicol-2
//! §columns). Сетка — `grid_shorthands`, линейки промежутков —
//! `rule_shorthands`. Части идут на места `ShorthandId::longhands`; опущенная
//! часть — начальное значение или копия по правилу сокращения.

use super::{LonghandId, ShorthandId};
use crate::document::state::ValueCx;
use crate::style::computed::split_outside_parens;
use LonghandId::*;

pub(super) const FLEX: &[LonghandId] = &[FlexGrow, FlexShrink, FlexBasis];
pub(super) const FLEX_FLOW: &[LonghandId] = &[FlexDirection, FlexWrap];
pub(super) const GAP: &[LonghandId] = &[RowGap, ColumnGap];
pub(super) const PLACE_CONTENT: &[LonghandId] = &[AlignContent, JustifyContent];
pub(super) const PLACE_ITEMS: &[LonghandId] = &[AlignItems, JustifyItems];
pub(super) const PLACE_SELF: &[LonghandId] = &[AlignSelf, JustifySelf];
pub(super) const COLUMNS: &[LonghandId] = &[ColumnWidth, ColumnCount, ColumnHeight, ColumnWrap];

impl ShorthandId {
    /// Длинные свойства сокращений раскладки, сетки и линеек.
    pub(super) fn layout_longhands(self) -> Option<&'static [LonghandId]> {
        Some(match self {
            Self::Flex => FLEX,
            Self::FlexFlow => FLEX_FLOW,
            Self::Gap => GAP,
            Self::PlaceContent => PLACE_CONTENT,
            Self::PlaceItems => PLACE_ITEMS,
            Self::PlaceSelf => PLACE_SELF,
            Self::Columns => COLUMNS,
            _ => return self.grid_longhands().or_else(|| self.rule_longhands()),
        })
    }

    /// Части сокращения раскладки, сетки или линеек по местам
    /// [`Self::layout_longhands`]; `None` — значение негодно.
    pub(super) fn layout_parts(self, value: &str, vcx: Option<ValueCx<'_>>) -> Option<Vec<String>> {
        match self {
            Self::Flex => flex(value, vcx),
            Self::FlexFlow => flex_flow(value),
            Self::Gap => gap(value),
            Self::PlaceContent | Self::PlaceItems | Self::PlaceSelf => place(self, value),
            Self::Columns => columns(value),
            _ if self.grid_longhands().is_some() => self.grid_parts(value),
            _ => self.rule_parts(value),
        }
    }
}

/// Число `<number [0,∞]>` (с документом — и математическая функция).
fn number(t: &str, vcx: Option<ValueCx<'_>>) -> Option<f64> {
    if let Ok(n) = t.parse::<f64>() {
        return (n >= 0.0 && n.is_finite()).then_some(n);
    }
    let n = crate::style::values::value::number(t, vcx?)?;
    (n >= 0.0).then_some(f64::from(n))
}

/// `flex: none | [ <'flex-grow'> <'flex-shrink'>? || <'flex-basis'> ]`
/// (css-flexbox-1 §7.1; разбор — Blink `Flex::ParseShorthand`). Опущенные
/// рост и сжатие — `1`, основа — `0%` (§7.1.1 «basic values of flex»);
/// основа не встаёт между ростом и сжатием; голый ноль после двух
/// множителей — основа.
fn flex(value: &str, vcx: Option<ValueCx<'_>>) -> Option<Vec<String>> {
    let toks = split_outside_parens(value.trim());
    if let [one] = toks.as_slice()
        && one.eq_ignore_ascii_case("none")
    {
        return Some(vec!["0".into(), "0".into(), "auto".into()]);
    }
    let (mut grow, mut shrink, mut basis): (Option<String>, Option<String>, Option<String>) =
        (None, None, None);
    for t in &toks {
        match number(t, vcx) {
            Some(_) if grow.is_none() => grow = Some(t.clone()),
            Some(_) if shrink.is_none() => shrink = Some(t.clone()),
            Some(n) if basis.is_none() && n == 0.0 && t.parse::<f64>().is_ok() => {
                basis = Some("0px".into());
            }
            Some(_) => return None,
            None if basis.is_none() => {
                basis = Some(t.clone());
                if grow.is_some() && shrink.is_none() {
                    shrink = Some("1".into());
                }
            }
            None => return None,
        }
    }
    if grow.is_none() && shrink.is_none() && basis.is_none() {
        return None;
    }
    Some(vec![
        grow.unwrap_or_else(|| "1".into()),
        shrink.unwrap_or_else(|| "1".into()),
        basis.unwrap_or_else(|| "0%".into()),
    ])
}

/// `flex-flow: <'flex-direction'> || <'flex-wrap'>` (css-flexbox-1 §5.3);
/// слова переноса (`wrap balance`) идут подряд.
fn flex_flow(value: &str) -> Option<Vec<String>> {
    super::text_shorthands::split(value, FLEX_FLOW, |w| match w {
        "row" | "row-reverse" | "column" | "column-reverse" => Some(0),
        "nowrap" | "wrap" | "wrap-reverse" | "balance" => Some(1),
        _ => None,
    })
}

/// `gap: <'row-gap'> <'column-gap'>?`: опущенный — копия первого.
fn gap(value: &str) -> Option<Vec<String>> {
    match split_outside_parens(value.trim()).as_slice() {
        [a] => Some(vec![a.clone(), a.clone()]),
        [a, b] => Some(vec![a.clone(), b.clone()]),
        _ => None,
    }
}

/// `place-*: <align> <justify>?` (css-align-3 §5.3, §6.3, §6.4): опущенное
/// второе — копия первого; у `place-content` первое `<baseline-position>`
/// даёт `justify-content: start`. Граница двух значений — единственная, где
/// оба годны своим свойствам (значения из нескольких слов: `safe center`,
/// `last baseline`).
fn place(id: ShorthandId, value: &str) -> Option<Vec<String>> {
    let [align, justify] = id.longhands() else {
        return None;
    };
    let ok = crate::style::computed::alignment_value;
    let words: Vec<&str> = value.split_whitespace().collect();
    for i in (1..=words.len()).rev() {
        let first = words[..i].join(" ");
        if !ok(*align, &first) {
            continue;
        }
        if i == words.len() {
            let baseline = first.to_ascii_lowercase().ends_with("baseline");
            let second = if id == ShorthandId::PlaceContent && baseline {
                "start".to_string()
            } else {
                first.clone()
            };
            if ok(*justify, &second) {
                return Some(vec![first, second]);
            }
            continue;
        }
        let second = words[i..].join(" ");
        if ok(*justify, &second) {
            return Some(vec![first, second]);
        }
    }
    None
}

/// `columns: [ <'column-width'> || <'column-count'> ] [ / <'column-height'> ]?`
/// (css-multicol-2 §columns) и сброс `column-wrap`. Положительное целое —
/// число колонок, ноль и длина — ширина; `auto` — опущенная сторона (Blink
/// `ConsumeColumnWidthOrCount`).
fn columns(value: &str) -> Option<Vec<String>> {
    let (head, tail) = split_slash(value);
    let words = split_outside_parens(head.trim());
    if words.is_empty() || words.len() > 2 || tail.is_some_and(str::is_empty) {
        return None;
    }
    let (mut width, mut count) = (None, None);
    for w in &words {
        if w.eq_ignore_ascii_case("auto") {
            continue;
        }
        let integer = w.strip_prefix('+').unwrap_or(w);
        let slot = if !integer.is_empty()
            && integer.bytes().all(|c| c.is_ascii_digit())
            && integer.bytes().any(|c| c != b'0')
        {
            &mut count
        } else {
            &mut width
        };
        if slot.replace(w.clone()).is_some() {
            return None;
        }
    }
    Some(vec![
        width.unwrap_or_else(|| "auto".into()),
        count.unwrap_or_else(|| "auto".into()),
        tail.map_or_else(|| "auto".into(), str::to_string),
        "auto".into(),
    ])
}

/// Разрезать значение по первой косой черте вне скобок.
pub(super) fn split_slash(v: &str) -> (&str, Option<&str>) {
    let mut depth = 0i32;
    for (i, ch) in v.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            '/' if depth == 0 => return (v[..i].trim(), Some(v[i + 1..].trim())),
            _ => {}
        }
    }
    (v.trim(), None)
}
