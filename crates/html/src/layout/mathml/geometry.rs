//! Pure logical-pixel metrics for MathML Core horizontal rows.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Metrics {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
}

pub(super) fn row(children: &[Metrics], rtl: bool) -> (Metrics, Vec<(f32, f32)>) {
    let metrics = Metrics {
        width: children.iter().map(|c| c.width).sum(),
        ascent: children.iter().map(|c| c.ascent).fold(0.0, f32::max),
        descent: children.iter().map(|c| c.descent).fold(0.0, f32::max),
    };
    let mut x = if rtl { metrics.width } else { 0.0 };
    let offsets = children
        .iter()
        .map(|c| {
            if rtl {
                x -= c.width;
            }
            let offset = (x, metrics.ascent - c.ascent);
            if !rtl {
                x += c.width;
            }
            offset
        })
        .collect();
    (metrics, offsets)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn row_aligns_baselines_and_preserves_dom_order_in_rtl() {
        let children = [
            Metrics {
                width: 4.0,
                ascent: 3.0,
                descent: 2.0,
            },
            Metrics {
                width: 7.0,
                ascent: 5.0,
                descent: 1.0,
            },
        ];
        let (metrics, offsets) = row(&children, false);
        assert_eq!(
            metrics,
            Metrics {
                width: 11.0,
                ascent: 5.0,
                descent: 2.0
            }
        );
        assert_eq!(offsets, [(0.0, 2.0), (4.0, 0.0)]);
        assert_eq!(row(&children, true).1, [(7.0, 2.0), (0.0, 0.0)]);
    }
    #[test]
    fn empty_row_has_a_real_zero_baseline() {
        assert_eq!(row(&[], false), (Metrics::default(), vec![]));
    }
}
