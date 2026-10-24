//! Зависящие от шрифта константы MathML, разрешаемые один раз при создании контекста.
use super::{fraction, limits, scripts};
use crate::style::computed::BoxStyle;
use crate::style::values::value::px_strict;
use crate::text::{fonts::math, inline::strut_font, metrics::math_metrics};
use crate::{dom::Element, layout::options::RenderOpts};

pub(super) struct FontParameters {
    pub fraction: fraction::Parameters,
    pub scripts: scripts::Parameters,
    pub limits: limits::Parameters,
    pub radical: super::radical::Parameters,
}
impl FontParameters {
    pub fn new(e: &Element, style: &BoxStyle, opts: &RenderOpts) -> Self {
        let font = strut_font(style, &opts.text, opts.cx.vcx());
        let size = px_strict(style.font_size).unwrap_or(opts.base_size());
        let (x, rule) = math_metrics(&font, size, opts.cx.vcx());
        let constants = math::for_family(&font.family, opts.cx.vcx());
        let c = |index: usize, fallback| {
            constants
                .as_ref()
                .map_or(fallback, |v| v.values[index] * size)
        };
        let display = style.math_compact == Some(false);
        let thickness = e
            .attr("linethickness")
            .and_then(|v| {
                if let Some(pct) = v.trim().strip_suffix('%') {
                    pct.parse::<f32>().ok().map(|v| v * c(34, rule) / 100.0)
                } else {
                    super::parameters::length_px(v, style, opts)
                }
            })
            .unwrap_or_else(|| c(34, rule))
            .max(0.0);
        let flags = e
            .children
            .iter()
            .find_map(|n| match n {
                crate::dom::Node::Element(child) => super::operators::core(child, style, opts),
                _ => None,
            })
            .map_or(0, |op| op.flags);
        let large = flags & 4 != 0;
        let stretch = flags & 17 == 17;
        let bar = !large && !stretch;
        Self {
            radical: super::radical::Parameters {
                gap: c(
                    if display { 46 } else { 45 },
                    rule + if display { x / 4.0 } else { rule / 4.0 },
                ),
                rule: c(47, rule),
                extra: c(48, rule),
                before: c(49, 5.0 * size / 18.0),
                after: c(50, -10.0 * size / 18.0),
                raise: constants.as_ref().map_or(0.6, |c| c.degree_raise),
                indexed: e.tag == "mroot",
            },
            fraction: fraction::Parameters {
                axis: c(1, x / 2.0),
                thickness,
                numerator_shift: c(if display { 29 } else { 28 }, 0.0),
                denominator_shift: c(if display { 31 } else { 30 }, 0.0),
                numerator_gap: c(
                    if display { 33 } else { 32 },
                    rule * if display { 3.0 } else { 1.0 },
                ),
                denominator_gap: c(
                    if display { 36 } else { 35 },
                    rule * if display { 3.0 } else { 1.0 },
                ),
                stack_top_shift: c(if display { 19 } else { 18 }, 0.0),
                stack_bottom_shift: c(if display { 21 } else { 20 }, 0.0),
                stack_gap: c(
                    if display { 23 } else { 22 },
                    rule * if display { 7.0 } else { 3.0 },
                ),
            },
            scripts: scripts::Parameters {
                sub_shift: c(4, x / 3.0),
                sub_drop: c(6, x / 2.0),
                sub_top: c(5, 4.0 * x / 5.0),
                sup_shift: c(if style.math_shift == Some(true) { 8 } else { 7 }, x),
                sup_drop: c(10, x / 2.0),
                sup_bottom: c(9, x / 4.0),
                sup_bottom_with_sub: c(12, 4.0 * x / 5.0),
                gap: c(11, size / 5.0),
                space: c(13, size / 5.0),
            },
            limits: limits::Parameters {
                under_gap: c(
                    if large {
                        16
                    } else if stretch {
                        27
                    } else {
                        42
                    },
                    3.0 * rule,
                ),
                over_gap: c(
                    if large {
                        14
                    } else if stretch {
                        26
                    } else {
                        39
                    },
                    3.0 * rule,
                ),
                under_shift: if large {
                    c(17, 0.0)
                } else if stretch {
                    c(25, 0.0)
                } else {
                    0.0
                },
                over_shift: if large {
                    c(15, 0.0)
                } else if stretch {
                    c(24, 0.0)
                } else {
                    0.0
                },
                under_extra: if bar { c(44, rule) } else { 0.0 },
                over_extra: if bar { c(41, rule) } else { 0.0 },
                accent_base: c(2, x / 2.0),
                accent_under: e.attr("accentunder") == Some("true"),
                accent_over: e.attr("accent") == Some("true"),
                bar_fallback: bar,
            },
        }
    }
}
