//! MathML Core §3.4.1: side scripts, preserving the base baseline.

use super::geometry::Metrics;

#[derive(Clone, Copy)]
pub(super) struct Parameters {
    pub sub_shift: f32,
    pub sub_drop: f32,
    pub sub_top: f32,
    pub sup_shift: f32,
    pub sup_drop: f32,
    pub sup_bottom: f32,
    pub sup_bottom_with_sub: f32,
    pub gap: f32,
    pub space: f32,
}

pub(super) fn scripts(
    base: Metrics,
    sub: Option<Metrics>,
    sup: Option<Metrics>,
    p: Parameters,
    rtl: bool,
) -> (Metrics, Vec<(f32, f32)>) {
    let mut down = sub.map_or(0.0, |s| {
        p.sub_shift
            .max(base.descent + p.sub_drop)
            .max(s.ascent - p.sub_top)
    });
    let mut up = sup.map_or(0.0, |s| {
        p.sup_shift
            .max(base.ascent - p.sup_drop)
            .max(s.descent + p.sup_bottom)
    });
    if let (Some(sub), Some(sup)) = (sub, sup) {
        let deficit = (p.gap - (down - sub.ascent + up - sup.descent)).max(0.0);
        let increase = deficit.min((p.sup_bottom_with_sub - up + sup.descent).max(0.0));
        up += increase;
        down += deficit - increase;
    }
    let ascent = base.ascent.max(sup.map_or(0.0, |s| up + s.ascent));
    let descent = base.descent.max(sub.map_or(0.0, |s| down + s.descent));
    let width = base.width
        + sub
            .map_or(0.0, |s| s.width)
            .max(sup.map_or(0.0, |s| s.width))
        + p.space;
    let x = |x, w| if rtl { width - x - w } else { x };
    let mut offsets = vec![(x(0.0, base.width), ascent - base.ascent)];
    if let Some(sub) = sub {
        offsets.push((x(base.width, sub.width), ascent + down - sub.ascent));
    }
    if let Some(sup) = sup {
        offsets.push((x(base.width, sup.width), ascent - up - sup.ascent));
    }
    (
        Metrics {
            width,
            ascent,
            descent,
        },
        offsets,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combined_scripts_resolve_gap_before_reflecting_positions() {
        let base = Metrics {
            width: 10.0,
            ascent: 8.0,
            descent: 2.0,
        };
        let script = Metrics {
            width: 4.0,
            ascent: 6.0,
            descent: 2.0,
        };
        let p = Parameters {
            sub_shift: 0.0,
            sub_drop: 0.0,
            sub_top: 5.0,
            sup_shift: 0.0,
            sup_drop: 8.0,
            sup_bottom: 0.0,
            sup_bottom_with_sub: 4.0,
            gap: 3.0,
            space: 1.0,
        };
        let (m, l) = scripts(base, Some(script), Some(script), p, false);
        assert_eq!(l[1].1 - l[2].1 - 8.0, 3.0);
        let (_, r) = scripts(base, Some(script), Some(script), p, true);
        assert_eq!(r[0].0, m.width - base.width);
        assert_eq!(r[1].0, 1.0);
        assert_eq!(r[1].1, l[1].1);
    }
}
