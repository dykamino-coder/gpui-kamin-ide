//! Разбор теней box-shadow/text-shadow: валидность и список теней.

use super::*;
use crate::document::state::ValueCx;

/// Годна ли запись `box-shadow` ЦЕЛИКОМ (css-backgrounds-3 §7.1:
/// `none | <shadow>#`; тень — 2-4 длины, не больше одного цвета и одного
/// `inset`). `none` внутри списка делает декларацию негодной.
pub(in crate::style::computed) fn box_shadow_valid(v: &str, vcx: ValueCx<'_>) -> bool {
    if v.trim().eq_ignore_ascii_case("none") {
        return true;
    }
    crate::style::css::split_args(v).iter().all(|s| {
        let (mut lens, mut colours, mut insets) = (0, 0, 0);
        for token in tokenize_shadow(s) {
            match Len::parse(&token, vcx) {
                Some(Len::Pct(_)) => return false,
                Some(_) => lens += 1,
                None if token.eq_ignore_ascii_case("inset") => insets += 1,
                None if token.eq_ignore_ascii_case("currentcolor")
                    || Color::parse(&token, vcx).is_some() =>
                {
                    colours += 1
                }
                None => return false,
            }
        }
        (2..=4).contains(&lens) && colours <= 1 && insets <= 1
    })
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::style::computed) struct ShadowValue {
    pub lengths: [Option<Len>; 4],
    pub color: Color,
    pub inset: bool,
}

impl ShadowValue {
    pub fn numeric(&self, vcx: ValueCx<'_>) -> Option<Shadow> {
        let mut values = [0.0; 4];
        for (slot, length) in values.iter_mut().zip(self.lengths) {
            *slot = match length {
                None => 0.0,
                Some(length @ (Len::Px(_) | Len::Calc(_))) => {
                    length.to_px(None, FontCx::FALLBACK, vcx)?
                }
                _ => return None,
            };
        }
        Some(Shadow {
            x: values[0],
            y: values[1],
            blur: values[2],
            spread: values[3],
            color: self.color,
        })
    }
}

/// Keep valid relative lengths for consumers that have a computed font context.
pub(in crate::style::computed) fn shadow_values(v: &str, vcx: ValueCx<'_>) -> Vec<ShadowValue> {
    let mut out = vec![];
    for s in crate::style::css::split_args(v) {
        let mut lengths = vec![];
        let mut color = None;
        for token in tokenize_shadow(s) {
            match Len::parse(&token, vcx) {
                Some(Len::Pct(_)) => return vec![],
                Some(Len::Calc(id))
                    if crate::style::values::value::calc_get(id, vcx).pct != 0.0 =>
                {
                    return vec![];
                }
                Some(length) => lengths.push(length),
                None => {
                    if let Some(c) = Color::parse(&token, vcx) {
                        color = Some(c);
                    } else if token != "currentcolor" && token != "inset" {
                        return vec![];
                    }
                }
            }
        }
        if !(2..=4).contains(&lengths.len()) {
            return vec![];
        }
        let mut value = ShadowValue {
            lengths: [None; 4],
            color: color.unwrap_or(Color {
                a: -1.0,
                ..Color::default()
            }),
            inset: s.contains("inset"),
        };
        for (slot, length) in value.lengths.iter_mut().zip(lengths) {
            *slot = Some(length);
        }
        out.push(value);
    }
    out
}

pub(in crate::style::computed) fn parse_shadows(v: &str, vcx: ValueCx<'_>) -> Vec<Shadow> {
    shadow_values(v, vcx)
        .iter()
        .filter(|s| !s.inset)
        .filter_map(|s| s.numeric(vcx))
        .collect()
}

/// Разбиение тени на токены: `rgba(0, 0, 0, .4)` — один токен, а не четыре.
fn tokenize_shadow(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch)
            }
            ')' => {
                depth -= 1;
                cur.push(ch)
            }
            c if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
