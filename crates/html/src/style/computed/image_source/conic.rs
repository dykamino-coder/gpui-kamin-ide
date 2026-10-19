//! Parsed conic rays, angular stop positions and interpolation operands.

use crate::document::state::ValueCx;
use crate::style::computed::{BgPos, GradSpace, Len};
use crate::style::values::{
    color_space::{ColorExpression, DirectGradient},
    value::Color,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ConicGradient {
    pub(crate) from: f32,
    pub(crate) position: BgPos,
    pub(crate) repeating: bool,
    pub(crate) stops: Vec<(Color, f32)>,
    pub(crate) hints: Vec<(usize, f32)>,
    pub(crate) space: GradSpace,
    pub(crate) hue: u8,
    pub(crate) polar: Option<DirectGradient>,
    colors: Vec<ColorExpression>,
}

impl ConicGradient {
    pub(crate) fn parse(raw: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let repeating = raw.starts_with("repeating-");
        let body = raw
            .strip_prefix("repeating-")
            .unwrap_or(raw)
            .strip_prefix("conic-gradient(")?
            .strip_suffix(')')?;
        let parts = crate::style::css::split_args(body);
        let mut idx = 0;
        let mut from = 0.0;
        let mut position = BgPos {
            x: Some(Len::Pct(0.5)),
            y: Some(Len::Pct(0.5)),
        };
        let mut interpolation = None;
        if let Some(head) = parts.first() {
            let words = crate::style::css::value_tokens::split_top(head.trim());
            if words
                .first()
                .is_some_and(|w| matches!(*w, "from" | "at" | "in"))
            {
                idx = 1;
                let mut i = 0;
                while i < words.len() {
                    match words[i] {
                        "from" => {
                            from = crate::style::values::value::angle::fraction(
                                words.get(i + 1)?,
                                vcx,
                            )
                            .unwrap_or(0.0);
                            i += 2;
                        }
                        "at" | "in" => {
                            let kind = words[i];
                            let start = i + 1;
                            i = start;
                            while i < words.len() && !matches!(words[i], "from" | "at" | "in") {
                                i += 1;
                            }
                            let value = words[start..i].join(" ");
                            if kind == "at" {
                                position = crate::style::computed::parse_pos_words(&value, vcx);
                            } else {
                                interpolation = Some(value);
                            }
                        }
                        _ => return None,
                    }
                }
            }
        }
        let mut colors = Vec::new();
        let mut raw_stops = Vec::new();
        let mut hints = Vec::new();
        let mut all_legacy = true;
        let black = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        for part in &parts[idx..] {
            let words = crate::style::computed::split_outside_parens(part);
            let Some(expression) = words.first().and_then(|w| ColorExpression::parse(w, vcx))
            else {
                if words.len() == 1
                    && let Some(at) = crate::style::values::value::angle::fraction(&words[0], vcx)
                {
                    hints.push((raw_stops.len(), at));
                }
                continue;
            };
            all_legacy &= crate::style::computed::gradient::legacy_srgb_color(&words[0])
                && !matches!(expression, ColorExpression::Relative(_));
            let color = expression.resolve_for_interpolation(black);
            let positions: Vec<_> = words[1..]
                .iter()
                .filter_map(|w| crate::style::values::value::angle::fraction(w, vcx))
                .collect();
            if positions.is_empty() {
                raw_stops.push((color, None));
                colors.push(expression);
            } else {
                for at in positions {
                    raw_stops.push((color, Some(at)));
                    colors.push(expression.clone());
                }
            }
        }
        if raw_stops.is_empty() {
            return None;
        }
        if raw_stops.len() == 1 {
            let color = raw_stops[0].0;
            raw_stops = vec![(color, Some(0.0)), (color, Some(1.0))];
            colors = vec![colors[0].clone(); 2];
        }
        let stops = crate::style::computed::place_stops(raw_stops);
        let (space, hue) = crate::style::computed::gradient::interpolation_method(
            interpolation.as_deref(),
            all_legacy,
        );
        let polar = DirectGradient::new(raw, stops.len(), space, vcx);
        Some(Self {
            from,
            position,
            repeating,
            stops,
            hints,
            space,
            hue,
            polar,
            colors,
        })
    }

    pub(crate) fn resolve_colors(&mut self, current: Color) {
        for ((color, _), expression) in self.stops.iter_mut().zip(&self.colors) {
            *color = expression.resolve_for_interpolation(current);
        }
    }

    pub(crate) fn resolve_lengths(&mut self, fix: impl Fn(&mut Option<Len>)) {
        fix(&mut self.position.x);
        fix(&mut self.position.y);
    }
}
