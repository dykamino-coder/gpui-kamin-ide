//! Resolve background position comparisons against concrete image free space.

mod expressions;
use super::{Computed, parse_pos_words};
use crate::style::values::value::Len;
use expressions::{capture, math, resolve};

impl Computed {
    pub(crate) fn set_background_position(&mut self, raw: &str) {
        self.bg_position_math = capture(raw);
        // A valid mixed comparison cannot yet become a linear Len. Keep a
        // placeholder for axis/edge classification; its used value comes from
        // the retained expression, so the other axis still parses correctly.
        let first = crate::style::css::split_args(raw)
            .into_iter()
            .next()
            .unwrap_or("");
        let words: Vec<String> = super::split_outside_parens(first)
            .into_iter()
            .map(|v| {
                if Len::parse_mixed(&v).is_none() && math(&v).is_some() {
                    "0px".into()
                } else {
                    v
                }
            })
            .collect();
        self.bg_pos = parse_pos_words(&words.join(" "));
        if self.bg_pos.x.is_some() || self.bg_pos.y.is_some() {
            self.accept_declaration();
        }
    }

    pub(crate) fn background_position_length(raw: &str) -> bool {
        Len::parse_mixed(raw).is_some() || math(raw).is_some()
    }

    pub(crate) fn set_background_position_axis(
        &mut self,
        axis: usize,
        raw: &str,
        value: Option<Len>,
    ) {
        let expression = math(raw);
        if value.is_some() || expression.is_some() {
            self.accept_declaration();
            let value = value.or_else(|| expression.as_ref().map(|_| Len::Px(0.0)));
            if axis == 0 {
                self.bg_pos.x = value.or(self.bg_pos.x);
            } else {
                self.bg_pos.y = value.or(self.bg_pos.y);
            }
            self.bg_position_math[axis] = expression;
        }
    }

    /// CSS Values 4 §10.2/10.10 compares resolved lengths, not percentage
    /// coefficients. Blink ResolveXPosition passes available width (which
    /// can be negative) into the calculated length evaluator.
    pub(crate) fn background_position_used(
        &self,
        fallback: (f32, f32),
        free: (f32, f32),
    ) -> (f32, f32) {
        if self.bg_position_math.iter().all(Option::is_none) {
            return fallback;
        }
        let font = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let (ch, ex) =
            crate::text::metrics::ch_ex_px(self.font_family.as_deref().unwrap_or(""), font);
        let line = match self.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) | Some(Len::Em(k)) => k * font,
            _ => {
                let (a, d, _) = crate::text::metrics::vmetrics_px(
                    self.font_family.as_deref().unwrap_or(""),
                    font,
                );
                a + d
            }
        };
        let mut used = [fallback.0, fallback.1];
        for (i, raw) in self.bg_position_math.iter().enumerate() {
            if let Some(raw) = raw {
                let raw = super::resolve::image_lengths::lengths(
                    raw,
                    font,
                    crate::style::values::value::root_font_px(),
                    ex,
                    ch,
                    line,
                );
                if let Some(value) = resolve(&raw, [free.0, free.1][i]) {
                    used[i] = value;
                }
            }
        }
        (used[0], used[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_free_space_changes_comparison_order_and_axis_overrides() {
        let mut c = Computed::default();
        c.set_background_position("min(0%, 100%) max(0%, 100%)");
        assert_eq!(
            c.background_position_used((123.0, 123.0), (-50.0, -50.0)),
            (-50.0, 0.0)
        );
        assert_eq!(
            c.background_position_used((123.0, 123.0), (50.0, 50.0)),
            (0.0, 50.0)
        );
        c.set_background_position_axis(0, "5px", Some(Len::Px(5.0)));
        assert_eq!(
            c.background_position_used((5.0, 123.0), (-50.0, -50.0)),
            (5.0, 0.0)
        );
        c.set_background_position("right min(100%, 10px) bottom max(100%, 10px)");
        assert_eq!(
            c.background_position_used((0.0, 0.0), (-50.0, -50.0)),
            (0.0, -60.0)
        );
    }

    #[test]
    fn computed_relative_lengths_survive_a_different_inherited_font() {
        let mut parent = Computed {
            font_size: Some(Len::Px(20.0)),
            line_height: Some(Len::Pct(2.0)),
            ..Computed::default()
        };
        parent.set_background_position("min(100%, 1em) max(100%, 1lh)");
        parent.resolve_image_lengths();
        let mut child = parent.clone();
        child.font_size = Some(Len::Px(80.0));
        child.line_height = Some(Len::Px(200.0));
        assert_eq!(
            child.background_position_used((0.0, 0.0), (50.0, 50.0)),
            (20.0, 50.0)
        );
    }
}
