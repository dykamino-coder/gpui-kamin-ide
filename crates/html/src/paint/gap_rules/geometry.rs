//! Геометрия правил промежутков: дорожки и прогоны.
mod assignment;
mod multicol;
#[cfg(test)]
mod span_tests {
    //! Zero-sized gutters distinguish touching items from actual spanners.
    use super::GapItem;
    use gpui::{Bounds, point, px, size};

    #[test]
    fn touching_a_zero_gap_does_not_span_it() {
        let item = |start: f32, end: f32| {
            GapItem::from_bounds(
                &Bounds {
                    origin: point(px(start), px(0.)),
                    size: size(px(end - start), px(1.)),
                },
                true,
            )
        };
        let left = item(0., 100.);
        let right = item(100., 200.);
        let spanner = item(0., 200.);
        assert!(!left.spans_a(100., 100.));
        assert!(!right.spans_a(100., 100.));
        assert!(spanner.spans_a(100., 100.));
        assert!(left.spans_a(80., 100.));
        assert!(spanner.spans_a(100., 120.));
        assert!(!left.spans_a(100., 120.));
    }
}
pub(super) use multicol::multicol_runs;
mod flex_lines;
pub(super) use flex_lines::FlexLines;
mod lane_visibility;
mod lines;
#[cfg(test)]
mod nan_order_tests;
pub(super) use lines::line_runs;

mod grid;
pub(super) use grid::grid_runs;
pub(super) use grid::subtract;

mod tracks;
pub(super) use tracks::GridTracks;
pub(super) use tracks::uncollapsed;

use gpui::{Bounds, Pixels};

pub(super) use crate::tolerance::GAP_RULE_EPS;

/// Элемент в осях `a` — поперёк промежутка, `b` — вдоль линейки.
#[derive(Clone, Copy, Debug)]
pub(super) struct GapItem {
    pub(crate) a0: f32,
    pub(crate) a1: f32,
    pub(crate) b0: f32,
    pub(crate) b1: f32,
    /// Used trailing margin on the stacking axis; box extents stay unchanged.
    pub(crate) margin_end: f32,
}

impl GapItem {
    pub(crate) fn from_bounds(b: &Bounds<Pixels>, gap_on_x: bool) -> Self {
        let x0 = f32::from(b.origin.x);
        let y0 = f32::from(b.origin.y);
        let x1 = x0 + f32::from(b.size.width);
        let y1 = y0 + f32::from(b.size.height);
        if gap_on_x {
            GapItem {
                a0: x0,
                a1: x1,
                b0: y0,
                b1: y1,
                margin_end: 0.0,
            }
        } else {
            GapItem {
                a0: y0,
                a1: y1,
                b0: x0,
                b1: x1,
                margin_end: 0.0,
            }
        }
    }

    pub(crate) fn flipped(&self) -> Self {
        GapItem {
            a0: self.b0,
            a1: self.b1,
            b0: self.a0,
            b1: self.a1,
            margin_end: 0.0,
        }
    }

    /// Заходит ли элемент в участок `[lo, hi]` вдоль линейки.
    pub(crate) fn covers_b(&self, lo: f32, hi: f32) -> bool {
        self.b0 < hi - GAP_RULE_EPS && self.b1 > lo + GAP_RULE_EPS
    }

    /// Перекрывает ли элемент промежуток `[g0, g1]` (спан через него).
    pub(crate) fn spans_a(&self, g0: f32, g1: f32) -> bool {
        if g1 - g0 <= GAP_RULE_EPS {
            // Touching a track boundary does not span the zero-sized gutter.
            return self.a0 < g0 - GAP_RULE_EPS && self.a1 > g1 + GAP_RULE_EPS;
        }
        self.a0 <= g0 + GAP_RULE_EPS && self.a1 >= g1 - GAP_RULE_EPS
    }
}

/// Пересекающий зазор на пути линейки: интервал вдоль `b`; рвёт ли он
/// линейку при `intersection` (видимое пересечение); есть ли в нём поперечная
/// линейка (стык, а не cap) и её ширина.
#[derive(Clone, Copy, Debug)]
pub(super) struct Crossing {
    pub(crate) lo: f32,
    pub(crate) hi: f32,
    pub(crate) breaks: bool,
    pub(crate) joins: bool,
    pub(crate) cross_w: f32,
}

/// Линейка одного промежутка: интервал промежутка `[g0, g1]`, протяжённость
/// `[r0, r1]`, пересечения, перекрытия спанами, скрытые по visibility участки
/// и характер концов протяжённости — стык (ширина зазора, есть ли линейка,
/// её ширина) или край контейнера (`None`).
#[derive(Clone, Debug)]
pub(super) struct GapRun {
    pub(crate) g0: f32,
    pub(crate) g1: f32,
    pub(crate) r0: f32,
    pub(crate) r1: f32,
    pub(crate) crossings: Vec<Crossing>,
    pub(crate) blocked: Vec<(f32, f32)>,
    pub(crate) hidden: Vec<(f32, f32)>,
    pub(crate) start_edge: Option<(f32, bool, f32)>,
    pub(crate) end_edge: Option<(f32, bool, f32)>,
    pub(crate) index: usize,
    pub(crate) count: usize,
}

impl GapRun {
    /// Конец отрезка: положение после отступа из пересекающего зазора к его
    /// границе, ширина зазора (0 у внешнего края), тип конца и ширина
    /// поперечной линейки. У внутреннего cap процентный inset сохраняет
    /// геометрическую ширину пересекающего зазора.
    pub(crate) fn edge(&self, pos: f32, is_start: bool) -> (f32, f32, bool, f32) {
        if is_start && (pos - self.r0).abs() <= GAP_RULE_EPS {
            return match self.start_edge {
                Some((cw, joins, dw)) => (self.r0, if joins { cw } else { 0.0 }, joins, dw),
                None => (self.r0, 0.0, false, 0.0),
            };
        }
        if !is_start && (pos - self.r1).abs() <= GAP_RULE_EPS {
            return match self.end_edge {
                Some((cw, joins, dw)) => (self.r1, if joins { cw } else { 0.0 }, joins, dw),
                None => (self.r1, 0.0, false, 0.0),
            };
        }
        if let Some(c) = self
            .crossings
            .iter()
            .find(|c| c.lo - GAP_RULE_EPS <= pos && pos <= c.hi + GAP_RULE_EPS)
        {
            let at = if is_start { c.hi } else { c.lo };
            return (at, c.hi - c.lo, c.joins, c.cross_w);
        }
        (pos, 0.0, false, 0.0)
    }
}

pub(super) fn uniq_sorted(mut v: Vec<f32>) -> Vec<f32> {
    v.sort_by(f32::total_cmp);
    v.dedup_by(|x, y| (*x - *y).abs() <= GAP_RULE_EPS);
    v
}

#[cfg(test)]
mod endpoints_tests {
    //! Crossing size is independent of the inset property selected at an endpoint.

    use super::{Crossing, GapRun};

    #[test]
    fn interior_caps_keep_the_geometric_crossing_width() {
        let mut run = GapRun {
            g0: 10.,
            g1: 30.,
            r0: 0.,
            r1: 220.,
            crossings: vec![Crossing {
                lo: 100.,
                hi: 120.,
                breaks: true,
                joins: false,
                cross_w: 3.,
            }],
            blocked: vec![],
            hidden: vec![],
            start_edge: None,
            end_edge: None,
            index: 0,
            count: 1,
        };
        assert_eq!(run.edge(100., false), (100., 20., false, 3.));
        assert_eq!(run.edge(120., true), (120., 20., false, 3.));
        assert_eq!(run.edge(0., true), (0., 0., false, 0.));
        assert_eq!(run.edge(220., false), (220., 0., false, 0.));
        run.crossings[0].joins = true;
        assert_eq!(run.edge(100., false), (100., 20., true, 3.));
    }
}
