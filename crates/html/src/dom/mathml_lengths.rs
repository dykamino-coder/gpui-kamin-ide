//! MathML Core mspace dimensions: CSS hints and independent requested ascent.

use crate::style::css::{Decls, parse_decls};
use crate::style::values::value::Len;

pub(super) fn hints(attrs: &[(String, String)]) -> Decls {
    let attr = |name| {
        attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| parse_mathml_length(v).map(|_| v.as_str()))
    };
    let mut hints = parse_decls("");
    if let Some(width) = attr("width") {
        hints.insert("width".into(), width.into());
    }
    let height = match (attr("height"), attr("depth")) {
        (Some(h), Some(d)) => format!("calc({h} + {d})"),
        (Some(v), None) | (None, Some(v)) => v.to_string(),
        (None, None) => "0px".into(),
    };
    hints.insert("height".into(), height);
    hints
}

pub(crate) fn parse_mathml_length(raw: &str) -> Option<Len> {
    let raw = raw.trim();
    if raw.parse::<f32>().is_ok_and(|v| v != 0.0) {
        return None;
    }
    let value = Len::parse(raw)?;
    (matches!(
        value,
        Len::Px(_)
            | Len::Em(_)
            | Len::Ex(_)
            | Len::Ch(_)
            | Len::Ic(_)
            | Len::Lh(_)
            | Len::Vw(_)
            | Len::Vh(_)
            | Len::EmPx(..)
            | Len::LhPx(..)
    ) || (matches!(value, Len::Calc(_)) && !raw.contains('(')))
    // A plain font-relative unit such as `cap` is stored as a calc sum; the
    // calc() function itself is not a MathML length.
    .then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn total_height_combines_valid_nonpercentage_attributes() {
        let attrs = vec![
            ("height".into(), "20px".into()),
            ("depth".into(), "30px".into()),
        ];
        assert_eq!(Len::parse(&hints(&attrs)["height"]), Some(Len::Px(50.0)));
        let attrs = vec![
            ("height".into(), "20%".into()),
            ("depth".into(), "2em".into()),
        ];
        assert_eq!(hints(&attrs)["height"], "2em");
        assert_eq!(hints(&[])["height"], "0px");
    }
    #[test]
    fn length_attributes_reject_percentages_bare_numbers_and_keywords() {
        for raw in ["20%", "20", "auto", "stretch", "min-content"] {
            assert!(parse_mathml_length(raw).is_none(), "{raw}");
        }
        assert_eq!(parse_mathml_length("-2em"), Some(Len::Em(-2.0)));
        assert_eq!(parse_mathml_length("0"), Some(Len::Px(0.0)));
    }
}
