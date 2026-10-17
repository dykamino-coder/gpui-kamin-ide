//! MathML Core stacked scripts; horizontal centering is independent of CSS width.
use super::geometry::Metrics;
#[derive(Clone, Copy)]
pub(super) struct Parameters {
    pub under_gap: f32,
    pub over_gap: f32,
    pub under_shift: f32,
    pub over_shift: f32,
    pub under_extra: f32,
    pub over_extra: f32,
    pub accent_base: f32,
    pub accent_under: bool,
    pub accent_over: bool,
    pub bar_fallback: bool,
}
pub(super) fn limits(
    base: Metrics,
    under: Option<Metrics>,
    over: Option<Metrics>,
    p: Parameters,
    width: Option<f32>,
) -> (Metrics, Vec<(f32, f32)>) {
    let natural = under.map_or(base.width, |v| v.width.max(base.width));
    let natural = over.map_or(natural, |v| v.width.max(natural));
    let width = width.unwrap_or(natural).max(0.0);
    let over_height = over.map_or(0.0, |v| v.ascent + v.descent);
    let top = over.map_or(0.0, |v| {
        if p.bar_fallback {
            over_height
                + if p.accent_over {
                    (p.accent_base - base.ascent).max(0.0)
                } else {
                    p.over_gap
                }
        } else {
            (over_height + p.over_gap).max(v.ascent + p.over_shift)
        }
    });
    let ascent = base.ascent + top + over.map_or(0.0, |_| p.over_extra);
    let bottom = under.map_or(0.0, |v| {
        if p.bar_fallback {
            if p.accent_under { 0.0 } else { p.under_gap }
        } else {
            p.under_gap.max(p.under_shift - v.ascent)
        }
    });
    let descent =
        base.descent + under.map_or(0.0, |v| bottom + v.ascent + v.descent + p.under_extra);
    let mut positions = vec![((width - base.width) / 2.0, ascent - base.ascent)];
    if let Some(v) = under {
        positions.push(((width - v.width) / 2.0, ascent + base.descent + bottom));
    }
    if let Some(v) = over {
        positions.push(((width - v.width) / 2.0, p.over_extra));
    }
    (
        Metrics {
            width,
            ascent,
            descent,
        },
        positions,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn centers_different_widths_without_changing_the_baseline() {
        let b = Metrics {
            width: 50.0,
            ascent: 15.0,
            descent: 0.0,
        };
        let u = Metrics { width: 75.0, ..b };
        let o = Metrics { width: 25.0, ..b };
        let p = Parameters {
            under_gap: 3.0,
            over_gap: 4.0,
            under_shift: 0.0,
            over_shift: 0.0,
            under_extra: 1.0,
            over_extra: 1.0,
            accent_base: 5.0,
            accent_under: false,
            accent_over: false,
            bar_fallback: true,
        };
        let (m, positions) = limits(b, Some(u), Some(o), p, None);
        assert_eq!((m.width, m.ascent, m.descent), (75.0, 35.0, 19.0));
        assert_eq!(positions, vec![(12.5, 20.0), (0.0, 38.0), (25.0, 1.0)]);
        let (wide, positions) = limits(b, Some(u), Some(o), p, Some(100.0));
        assert_eq!((wide.ascent, wide.descent), (m.ascent, m.descent));
        assert_eq!(positions[0].0, 25.0);
    }
}
