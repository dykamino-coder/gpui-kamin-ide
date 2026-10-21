//! Разбор экспериментального неотрицательного CSS-отображения длины SVG pathLength (Blink).

use crate::document::state::ValueCx;
use crate::style::values::value::{Len, calc_get};

pub(super) fn parse(raw: &str, vcx: ValueCx<'_>) -> Option<Len> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("none") {
        return Some(Len::Auto);
    }
    if raw.parse::<f32>().is_ok_and(|v| v != 0.0) {
        return None;
    }
    let length = Len::parse_mixed(raw, vcx)?;
    let math = raw.contains('(');
    let valid_number = |v: f32| v.is_finite() && (v >= 0.0 || math);
    match length {
        Len::Px(v) if valid_number(v) => Some(Len::Px(v.max(0.0))),
        Len::Em(v)
        | Len::Ch(v)
        | Len::Ex(v)
        | Len::Ic(v)
        | Len::Lh(v)
        | Len::Vw(v)
        | Len::Vh(v)
            if valid_number(v) =>
        {
            Some(length)
        }
        Len::EmPx(a, b) | Len::LhPx(a, b) if valid_number(a) && valid_number(b) => Some(length),
        Len::Calc(id) => {
            let s = calc_get(id, vcx);
            (!s.has_percentage
                && s.pct == 0.0
                && [s.px, s.em, s.ch, s.ex, s.ic, s.cap, s.lh, s.vw, s.vh]
                    .into_iter()
                    .chain(s.ext)
                    .all(valid_number))
            .then_some(length)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_grammar_and_range() {
        let doc = crate::document::state::DocState::new(Default::default());
        for raw in ["10px", "0", "1in", "none", "2em", "5vw", "calc(1em + 2px)"] {
            assert!(parse(raw, doc.values()).is_some(), "{raw}");
        }
        for raw in [
            "-1px",
            "-2em",
            "10%",
            "calc(10% - 10% + 2px)",
            "10",
            "auto",
            "NaNpx",
            "infinitypx",
        ] {
            assert_eq!(parse(raw, doc.values()), None, "{raw}");
        }
    }
}
