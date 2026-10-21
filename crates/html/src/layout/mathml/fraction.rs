//! MathML Core §3.3.2: дроби с чертой и стопки без черты, без отрисовки.

use super::geometry::Metrics;

#[derive(Clone, Copy)]
pub(super) struct Parameters {
    pub axis: f32,
    pub thickness: f32,
    pub numerator_shift: f32,
    pub denominator_shift: f32,
    pub numerator_gap: f32,
    pub denominator_gap: f32,
    pub stack_top_shift: f32,
    pub stack_bottom_shift: f32,
    pub stack_gap: f32,
}

pub(super) struct Layout {
    pub metrics: Metrics,
    pub offsets: [(f32, f32); 2],
    pub rule: Option<(f32, f32, f32, f32)>,
}

pub(super) fn fraction(n: Metrics, d: Metrics, p: Parameters, width: Option<f32>) -> Layout {
    let thickness = p.thickness.max(0.0);
    let (up, down) = if thickness > 0.0 {
        (
            p.numerator_shift
                .max(p.axis + thickness / 2.0 + p.numerator_gap + n.descent),
            p.denominator_shift
                .max(thickness / 2.0 + p.denominator_gap + d.ascent - p.axis),
        )
    } else {
        let gap = p.stack_top_shift + p.stack_bottom_shift - n.descent - d.ascent;
        let increase = (p.stack_gap - gap).max(0.0) / 2.0;
        (
            p.stack_top_shift + increase,
            p.stack_bottom_shift + increase,
        )
    };
    let metrics = Metrics {
        width: width.unwrap_or(n.width.max(d.width)).max(0.0),
        ascent: (up + n.ascent)
            .max(d.ascent - down)
            .max(if thickness > 0.0 {
                p.axis + thickness / 2.0
            } else {
                0.0
            }),
        descent: (n.descent - up)
            .max(down + d.descent)
            .max(if thickness > 0.0 {
                thickness / 2.0 - p.axis
            } else {
                0.0
            }),
    };
    Layout {
        metrics,
        offsets: [
            (
                (metrics.width - n.width) / 2.0,
                metrics.ascent - up - n.ascent,
            ),
            (
                (metrics.width - d.width) / 2.0,
                metrics.ascent + down - d.ascent,
            ),
        ],
        rule: (thickness > 0.0).then_some((
            0.0,
            metrics.ascent - p.axis - thickness / 2.0,
            metrics.width,
            thickness,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn b(width: f32, ascent: f32, descent: f32) -> Metrics {
        Metrics {
            width,
            ascent,
            descent,
        }
    }
    fn parameters() -> Parameters {
        Parameters {
            axis: 3.0,
            thickness: 2.0,
            numerator_shift: 0.0,
            denominator_shift: 0.0,
            numerator_gap: 4.0,
            denominator_gap: 5.0,
            stack_top_shift: 0.0,
            stack_bottom_shift: 0.0,
            stack_gap: 6.0,
        }
    }
    #[test]
    fn rule_clearances_and_css_width_are_independent() {
        let l = fraction(
            b(20.0, 8.0, 2.0),
            b(10.0, 7.0, 3.0),
            parameters(),
            Some(40.0),
        );
        assert_eq!(l.metrics, b(40.0, 18.0, 13.0));
        let (_, y, _, h) = l.rule.unwrap();
        assert_eq!(y - (l.offsets[0].1 + 10.0), 4.0);
        assert_eq!(l.offsets[1].1 - y - h, 5.0);
        assert_eq!((l.offsets[0].0, l.offsets[1].0), (10.0, 15.0));
    }
    #[test]
    fn zero_rule_splits_extra_gap_equally() {
        let mut p = parameters();
        p.thickness = 0.0;
        let l = fraction(b(20.0, 8.0, 2.0), b(10.0, 7.0, 3.0), p, None);
        assert!(l.rule.is_none());
        assert_eq!(l.offsets[1].1 - l.offsets[0].1 - 10.0, 6.0);
        assert_eq!(l.metrics, b(20.0, 15.5, 10.5));
    }
}
