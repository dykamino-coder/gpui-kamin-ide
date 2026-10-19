//! Контур, сдвинутый от края коробки (css-borders-4 §corner-shaping: тени
//! `box-shadow` с разлётом, `outline-offset` и обрезка переполнения идут по
//! ОДНОЙ форме — контуру, раздвинутому или сжатому на заданные толщины).
//! Перенос Blink `ContouredRect` с исходным прямоугольником (`origin rect`):
//! целевой прямоугольник без выровненных по исходным углам вырезов
//! (`CornerClipOutPath`) — тот же расчёт, что у внутреннего контура рамки,
//! только толщины могут быть отрицательными (наружу).

use super::{Path, Seg, Shaped, aligned};

/// Контур коробки `s`, сдвинутый внутрь на t/r/b/l (отрицательные — наружу),
/// одним замкнутым путём: стороны прямоугольника и цепочки углов.
pub fn offset(s: &Shaped, insets: [f32; 4]) -> Path {
    let target = s.inset_signed(insets);
    let (origin_c, target_c) = (s.corners(), target.corners());
    let [t, r, b, l] = insets;
    let edges = [(t, r), (r, b), (b, l), (l, t)];
    let mut start = None;
    let mut segs = Vec::new();
    for i in 0..4 {
        let (head, chain) =
            match aligned::clip_out(&origin_c[i], &target_c[i], edges[i].0, edges[i].1) {
                Some((ms, chain)) => (ms, chain),
                None => (target_c[i].outer(), Vec::new()),
            };
        match start {
            None => start = Some(head),
            Some(_) => segs.push(Seg::Line(head)),
        }
        segs.extend(chain);
    }
    let start = start.unwrap_or((0.0, 0.0));
    // Замыкание: последний отрезок в начальную точку не нужен (`Z`).
    if segs.last() == Some(&Seg::Line(start)) {
        segs.pop();
    }
    Path { start, segs }
}

impl Shaped {
    /// Прямоугольник, сдвинутый на t/r/b/l любого знака; радиусы — за
    /// вычетом толщин с полом 0 (`FloatRoundedRect::Outset`/`Inset`).
    pub(super) fn inset_signed(&self, [t, r, b, l]: [f32; 4]) -> Shaped {
        let (x, y, w, h) = self.rect;
        let [tl, tr, br, bl] = self.radii;
        let s = |(rx, ry): (f32, f32), dx: f32, dy: f32| {
            if rx <= 0.0 || ry <= 0.0 {
                (0.0, 0.0)
            } else {
                ((rx - dx).max(0.0), (ry - dy).max(0.0))
            }
        };
        Shaped {
            rect: (x + l, y + t, (w - l - r).max(0.0), (h - t - b).max(0.0)),
            radii: [s(tl, l, t), s(tr, r, t), s(br, r, b), s(bl, l, b)],
            k: self.k,
        }
    }
}
