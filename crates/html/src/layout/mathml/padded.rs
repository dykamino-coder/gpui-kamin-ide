//! Геометрия mpadded из MathML Core после разрешения атрибутов length-percentage.
use super::geometry::Metrics;
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Parameters {
    pub width: Option<f32>,
    pub ascent: Option<f32>,
    pub descent: Option<f32>,
    pub lspace: f32,
    pub voffset: f32,
    pub css_width: Option<f32>,
    pub css_height: Option<f32>,
    pub rtl: bool,
}
pub(super) fn padded(inner: Metrics, p: Parameters) -> (Metrics, [f32; 2]) {
    let width = p.css_width.or(p.width).unwrap_or(inner.width).max(0.0);
    let ascent = p.ascent.unwrap_or(inner.ascent).max(0.0);
    let requested_descent = p.descent.unwrap_or(inner.descent).max(0.0);
    let descent = p
        .css_height
        .map_or(requested_descent, |height| height.max(0.0) - ascent);
    let lspace = p.lspace.max(0.0);
    let x = if p.rtl {
        width - inner.width - lspace
    } else {
        lspace
    };
    (
        Metrics {
            width,
            ascent,
            descent,
        },
        [x, ascent - inner.ascent - p.voffset],
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requested_metrics_and_voffset_do_not_change_inner_metrics() {
        let inner = Metrics {
            width: 20.0,
            ascent: 10.0,
            descent: 10.0,
        };
        let (m, offset) = padded(
            inner,
            Parameters {
                width: Some(100.0),
                ascent: Some(50.0),
                descent: Some(50.0),
                lspace: 20.0,
                voffset: 30.0,
                ..Parameters::default()
            },
        );
        assert_eq!((m.width, m.ascent, m.descent), (100.0, 50.0, 50.0));
        assert_eq!(offset, [20.0, 10.0]);
    }
    #[test]
    fn css_height_preserves_ascent_and_rtl_uses_the_used_width() {
        let (m, offset) = padded(
            Metrics {
                width: 20.0,
                ascent: 10.0,
                descent: 10.0,
            },
            Parameters {
                width: Some(100.0),
                ascent: Some(50.0),
                css_width: Some(80.0),
                css_height: Some(20.0),
                lspace: 15.0,
                rtl: true,
                ..Parameters::default()
            },
        );
        assert_eq!((m.width, m.ascent, m.descent), (80.0, 50.0, -30.0));
        assert_eq!(offset, [45.0, 40.0]);
    }
}
