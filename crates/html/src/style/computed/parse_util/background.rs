//! Разбор фона: проверка списка `background-blend-mode`, список слоёв через
//! запятую, пары процент/точки.

use super::*;
use crate::document::state::ValueCx;

/// Validate the complete blend list before the cascade stores its first value.
pub(crate) fn background_blend_valid(raw: &str) -> bool {
    if matches!(
        raw.trim(),
        "inherit" | "initial" | "unset" | "revert" | "revert-layer"
    ) {
        return true;
    }
    background_layers(raw)
        .iter()
        .all(|value| BlendMode::parse(value.trim()).is_some_and(|mode| mode != BlendMode::Inherit))
}

/// Слои фона: значение режется по запятым ВНЕ скобок.
///
/// Запятая внутри `rgba(…)` или `linear-gradient(…)` слой не кончает, поэтому
/// делить строку простым `split(',')` нельзя.
pub(crate) fn background_layers(v: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut rest = v;
    while let Some(at) = top_level_comma(rest) {
        out.push(rest[..at].trim());
        rest = &rest[at + 1..];
    }
    out.push(rest.trim());
    out
}

pub(crate) fn top_level_comma(inner: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Процентная смесь `calc(A% ± Bpx)` парой (доля, точки) — для свойств,
/// которые доли решают САМИ при отрисовке, зная размер коробки
/// (css-values-4 §10.9). Любая другая природа в сумме (`ch`, `vw`, `em`…) —
/// `None`: её здесь сложить не с чем, и запись, как прежде, не применяется.
pub(in crate::style::computed) fn pct_px_pair(t: &str, vcx: ValueCx<'_>) -> Option<(f32, f32)> {
    match Len::parse_mixed(t, vcx)? {
        Len::Calc(i) => crate::style::values::value::calc_get(i, vcx).pct_px(),
        _ => None,
    }
}
