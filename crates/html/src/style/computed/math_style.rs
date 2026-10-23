//! Вычисленные MathML style/depth и математическая шкала размеров из CSS Fonts.
use super::Computed;
use crate::{style::values::value::Len, text::fonts};
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Depth {
    Absolute(i32),
    Add(i32),
    Auto,
}
pub(crate) fn depth(value: &str) -> Option<Depth> {
    if value == "auto-add" {
        Some(Depth::Auto)
    } else if let Some(v) = value.strip_prefix("add(").and_then(|v| v.strip_suffix(')')) {
        v.trim().parse().ok().map(Depth::Add)
    } else {
        value.parse().ok().map(Depth::Absolute)
    }
}
pub(crate) fn inherit(
    parent: &Computed,
    c: &mut Computed,
    vcx: crate::document::state::ValueCx<'_>,
) {
    c.math_compact = c.math_compact.or(parent.math_compact);
    c.math_shift = c.math_shift.or(parent.math_shift);
    c.math_operator_form = c.math_operator_form.or(parent.math_operator_form);
    c.math_level = match c.math_depth {
        Some(Depth::Absolute(v)) => v,
        Some(Depth::Add(v)) => parent.math_level.saturating_add(v),
        Some(Depth::Auto) => parent
            .math_level
            .saturating_add(i32::from(parent.math_compact == Some(true))),
        None => parent.math_level,
    };
    if c.math_font_size {
        let family = parent.font_family.as_deref().unwrap_or("");
        let actual = fonts::alias_for(family, parent, vcx).unwrap_or_else(|| family.to_string());
        let constants = fonts::math::for_family(&actual, vcx);
        let factor = scale(
            parent.math_level,
            c.math_level,
            constants
                .as_ref()
                .map(|v| (v.script_scale, v.script_script_scale)),
        );
        let size = if let Some(Len::Px(v)) = parent.font_size {
            v
        } else {
            16.0
        };
        c.font_size = Some(Len::Px(size * factor));
    }
}
fn scale(mut a: i32, mut b: i32, constants: Option<(f32, f32)>) -> f32 {
    if a == b {
        return 1.0;
    }
    let invert = b < a;
    if invert {
        std::mem::swap(&mut a, &mut b);
    }
    let mut e = b.saturating_sub(a);
    let mut s = 1.0;
    if let Some((script, scriptscript)) = constants.filter(|(s, ss)| *s > 0.0 && *ss > 0.0) {
        if a <= 0 && b >= 2 {
            s *= scriptscript;
            e -= 2;
        } else if a == 1 {
            s *= scriptscript / script;
            e -= 1;
        } else if b == 1 {
            s *= script;
            e -= 1;
        }
    }
    s *= 0.71f32.powi(e);
    if invert { 1.0 / s } else { s }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn css_wide_style_reset_controls_auto_depth_font_size() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        use crate::style::{cascade::inherit::inherit, css::parse_decls};
        let parent = {
            let mut with_inherited = Computed {
                ..Computed::default()
            };
            with_inherited.math_compact = Some(true);
            with_inherited.font_size = Some(Len::Px(500.0));
            with_inherited.font_family = Some("Ahem".into());
            with_inherited
        };
        for (reset, compact, size) in [("initial", false, 500.0), ("unset", true, 355.0)] {
            let own = Computed::resolve(&[], &parse_decls(&format!("math-style: {reset}")), vcx);
            let middle = inherit(&parent, &own, vcx);
            assert_eq!(middle.math_compact, Some(compact));
            let own = Computed::resolve(
                &[],
                &parse_decls("font-size: math; math-depth: auto-add"),
                vcx,
            );
            let child = inherit(&middle, &own, vcx);
            let Some(Len::Px(actual)) = child.font_size else {
                panic!("computed pixels")
            };
            assert!((actual - size).abs() < 0.001, "{reset}: {actual}");
        }
    }
    #[test]
    fn math_scale_uses_two_font_constants_and_reverses_depth() {
        assert_eq!(scale(0, 2, Some((0.8, 0.6))), 0.6);
        assert_eq!(scale(1, 2, Some((0.8, 0.6))), 0.75);
        assert_eq!(scale(2, 0, Some((0.8, 0.6))), 1.0 / 0.6);
        assert_eq!(scale(0, 3, None), 0.71f32.powi(3));
    }
}
