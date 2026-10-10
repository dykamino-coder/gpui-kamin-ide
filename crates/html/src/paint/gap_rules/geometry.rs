//! Геометрия правил промежутков: дорожки и прогоны.
mod lines;
pub(super) use lines::line_runs;

mod grid;
pub(super) use grid::grid_runs;
pub(super) use grid::subtract;

mod tracks;
pub(super) use tracks::GridTracks;
pub(super) use tracks::uncollapsed;

// owner: A

use gpui::{Bounds, Pixels};

/// Допуск сравнения координат раскладки.
pub(super) const GAP_EPS: f32 = 0.35;

/// Элемент в осях `a` — поперёк промежутка, `b` — вдоль линейки.
#[derive(Clone, Copy, Debug)]
pub(super) struct GapItem {
    pub(crate) a0: f32,
    pub(crate) a1: f32,
    pub(crate) b0: f32,
    pub(crate) b1: f32,
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
            }
        } else {
            GapItem {
                a0: y0,
                a1: y1,
                b0: x0,
                b1: x1,
            }
        }
    }

    pub(crate) fn flipped(&self) -> Self {
        GapItem {
            a0: self.b0,
            a1: self.b1,
            b0: self.a0,
            b1: self.a1,
        }
    }

    /// Заходит ли элемент в участок `[lo, hi]` вдоль линейки.
    pub(crate) fn covers_b(&self, lo: f32, hi: f32) -> bool {
        self.b0 < hi - GAP_EPS && self.b1 > lo + GAP_EPS
    }

    /// Перекрывает ли элемент промежуток `[g0, g1]` (спан через него).
    pub(crate) fn spans_a(&self, g0: f32, g1: f32) -> bool {
        self.a0 <= g0 + GAP_EPS && self.a1 >= g1 - GAP_EPS
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
    /// границе, ширина зазора (0 у края и у «висячего» конца без поперечной
    /// линейки — так считает Blink `GetMaxInsetWidth`), есть ли стык и ширина
    /// поперечной линейки.
    pub(crate) fn edge(&self, pos: f32, is_start: bool) -> (f32, f32, bool, f32) {
        if is_start && (pos - self.r0).abs() <= GAP_EPS {
            return match self.start_edge {
                Some((cw, joins, dw)) => (self.r0, if joins { cw } else { 0.0 }, joins, dw),
                None => (self.r0, 0.0, false, 0.0),
            };
        }
        if !is_start && (pos - self.r1).abs() <= GAP_EPS {
            return match self.end_edge {
                Some((cw, joins, dw)) => (self.r1, if joins { cw } else { 0.0 }, joins, dw),
                None => (self.r1, 0.0, false, 0.0),
            };
        }
        if let Some(c) = self
            .crossings
            .iter()
            .find(|c| c.lo - GAP_EPS <= pos && pos <= c.hi + GAP_EPS)
        {
            let at = if is_start { c.hi } else { c.lo };
            return (
                at,
                if c.joins { c.hi - c.lo } else { 0.0 },
                c.joins,
                c.cross_w,
            );
        }
        (pos, 0.0, false, 0.0)
    }
}

pub(super) fn uniq_sorted(mut v: Vec<f32>) -> Vec<f32> {
    v.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    v.dedup_by(|x, y| (*x - *y).abs() <= GAP_EPS);
    v
}
