//! HSL-цвета: раздельные грамматики legacy (через запятую) и современная (пробел/слэш).

use super::{Color, css_number, fold_simple_calc};

pub(super) fn parse(inner: &str) -> Option<Color> {
    // CSS Color 4 §7: legacy syntax requires percentages and forbids none;
    // modern syntax allows missing components and uses a slash for alpha.
    let folded = fold_simple_calc(inner).to_ascii_lowercase();
    let legacy = folded.contains(',');
    let parts: Vec<&str> = if legacy {
        if folded.contains('/') {
            return None;
        }
        folded.split(',').map(str::trim).collect()
    } else {
        let slash: Vec<_> = folded.split('/').collect();
        if slash.len() > 2 {
            return None;
        }
        let mut parts: Vec<_> = slash[0].split_ascii_whitespace().collect();
        if parts.len() != 3 {
            return None;
        }
        if slash.len() == 2 {
            parts.push(slash[1].trim());
        }
        parts
    };
    if !(3..=4).contains(&parts.len()) {
        return None;
    }
    let missing = |value: &str| !legacy && value == "none";
    let hue = if missing(parts[0]) {
        0.0
    } else {
        let angle = parts[0];
        let (number, scale) = if let Some(n) = angle.strip_suffix("grad") {
            (n, 400.0)
        } else if let Some(n) = angle.strip_suffix("rad") {
            (n, std::f32::consts::TAU)
        } else if let Some(n) = angle.strip_suffix("turn") {
            (n, 1.0)
        } else {
            (angle.strip_suffix("deg").unwrap_or(angle), 360.0)
        };
        css_number(number)? / scale
    }
    .rem_euclid(1.0);
    let component = |value: &str| -> Option<f32> {
        if missing(value) {
            return Some(0.0);
        }
        let number = match value.strip_suffix('%') {
            Some(number) => number,
            None if !legacy => value,
            None => return None,
        };
        Some((css_number(number)? / 100.0).clamp(0.0, 1.0))
    };
    let alpha = match parts.get(3) {
        None => 1.0,
        Some(value) if missing(value) => 0.0,
        Some(value) => match value.strip_suffix('%') {
            Some(number) => css_number(number)? / 100.0,
            None => css_number(value)?,
        }
        .clamp(0.0, 1.0),
    };
    let rgba: gpui::Rgba =
        gpui::hsla(hue, component(parts[1])?, component(parts[2])?, alpha).into();
    Some(Color {
        r: rgba.r,
        g: rgba.g,
        b: rgba.b,
        a: rgba.a,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_and_legacy_forms_have_exact_components() {
        for value in [
            "0, 100%, 50%",
            "0 100% 50%",
            "0\t100%\n50%",
            "0 100 50",
            "1turn 100% 50%",
            "calc(180deg * 2) 100% 50%",
        ] {
            assert_eq!(
                parse(value),
                Some(Color {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0
                }),
                "{value}"
            );
        }
        assert_eq!(parse("none none none / none"), Some(Color::default()));
        assert_eq!(parse("0, 100%, 50%, 50%").unwrap().a, 0.5);
        assert_eq!(parse("0 100% 50% / 2").unwrap().a, 1.0);
    }

    #[test]
    fn invalid_hsl_is_rejected_instead_of_truncated() {
        for value in [
            "0,100%,50%,1,2",
            "0 100% 50% 1",
            "0,100%,50% / .5",
            "0 100% 50% / 1 / 2",
            "0,,100%,50%",
            "0,100%,50%,",
            "none,100%,50%",
            "0,100,50",
            "0degdeg 100% 50%",
            "0 100%% 50%",
            "NaN 100% 50%",
            "0 100% 50% / infinity",
            "0 100% 50% / .5 extra",
        ] {
            assert_eq!(parse(value), None, "{value}");
        }
    }
}
