//! Разбор сросшихся кромок (CSS 2.1 §17.6.2.1): кандидаты по линиям сетки и победители по отрезкам.

use super::{EdgeCell, GRID_BOX};
use gpui::{Bounds, Pixels};

// Кандидат кромки на ЛИНИИ сетки: совпадающие отрезки соседей — ОДНА
// кромка, победитель по CSS 2.1 §17.6.2.1 (hidden гасит всех, затем
// шире, ранг стиля, источник ячейка>таблица, порядок в документе).
pub(super) struct Cand {
    pub(super) line: f32,
    pub(super) a: f32,
    pub(super) b: f32,
    pub(super) w: f32,
    pub(super) style: u8,
    pub(super) source: u8,
    pub(super) doc_ix: u32,
    pub(super) colour: crate::style::values::value::Color,
    /// Наружная сторона крайней линии таблицы (-1/1); 0 — центр.
    pub(super) outward: i8,
}

pub(super) type SegKey = (f32, u8, u8, u32);

/// Total order of segment keys: width first (`f32::total_cmp`, so a NaN
/// width cannot break the sort), then style, source and document order.
pub(super) fn key_order(p: &SegKey, q: &SegKey) -> std::cmp::Ordering {
    p.0.total_cmp(&q.0)
        .then((p.1, p.2, p.3).cmp(&(q.1, q.2, q.3)))
}

impl Cand {
    fn key(&self) -> SegKey {
        (self.w, self.style, self.source, u32::MAX - self.doc_ix)
    }
}

pub(super) type Seg = (
    SegKey,
    bool,
    Bounds<Pixels>,
    crate::style::values::value::Color,
);

/// Кандидаты кромок по линиям сетки: вертикали и горизонтали всех проб, кроме рамки сетки.
pub(super) fn collect_cands(cells: &[EdgeCell]) -> (Vec<Cand>, Vec<Cand>) {
    let mut vert: Vec<Cand> = vec![];
    let mut horiz: Vec<Cand> = vec![];
    for c in cells {
        if c.source == GRID_BOX {
            continue;
        }
        let bnd = c.bounds;
        let (x0, y0) = (f32::from(bnd.origin.x), f32::from(bnd.origin.y));
        let (x1, y1) = (
            x0 + f32::from(bnd.size.width),
            y0 + f32::from(bnd.size.height),
        );
        let is_table = c.source == 0;
        let side = |list: &mut Vec<Cand>, line: f32, a: f32, b: f32, i: usize, out: i8| {
            if c.widths[i] > 0.0 || c.styles[i] == 1 {
                list.push(Cand {
                    line,
                    a,
                    b,
                    w: c.widths[i],
                    style: c.styles[i],
                    source: c.source,
                    doc_ix: c.doc_ix,
                    colour: c.colors[i],
                    outward: if is_table { out } else { 0 },
                });
            }
        };
        side(&mut horiz, y0, x0, x1, 0, -1);
        side(&mut vert, x1, y0, y1, 1, 1);
        side(&mut horiz, y1, x0, x1, 2, 1);
        side(&mut vert, x0, y0, y1, 3, -1);
    }
    (vert, horiz)
}

/// Отрезки кромок одной оси: победитель на каждом куске линии (§17.6.2.1) копится в `segs`.
pub(super) fn draw_cands(
    cands: &mut [Cand],
    vertical: bool,
    grid_lo: Option<f32>,
    half_at: &dyn Fn(f32, f32) -> f32,
    half_at_h: &dyn Fn(f32, f32) -> f32,
    segs: &mut Vec<Seg>,
) {
    cands.sort_by(|p, q| p.line.total_cmp(&q.line));
    // Крайние линии таблицы: кромка не центрируется, а рисуется
    // внутрь бокса (наружная половина у браузеров уходит в поля,
    // эталоны считают рамку частью коробки).
    // Начало сетки — не «первая нарисованная кромка», а край ДОРОЖЕК.
    // У браузера коробка таблицы раздаётся наружу на половину
    // победившей кромки, а сама кромка красится центрировано. Выноса
    // у нас нет: сетка стоит там, где у браузера ВНЕШНИЙ край
    // коробки, — поэтому кромку НАЧАЛЬНОЙ линии вжимаем внутрь, её
    // наружная половина и занимает недостающий вынос. Конец сетки
    // координату не сдвигает: там центр.
    let lo_line = grid_lo.unwrap_or_else(|| cands.first().map(|c| c.line).unwrap_or(0.0));
    let mut i = 0;
    while i < cands.len() {
        let mut j = i + 1;
        while j < cands.len() && (cands[j].line - cands[i].line).abs() < 0.75 {
            j += 1;
        }
        let group = &cands[i..j];
        let outward = group
            .iter()
            .find_map(|c| (c.outward != 0).then_some(c.outward));
        let mut cuts: Vec<f32> = group.iter().flat_map(|c| [c.a, c.b]).collect();
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|p, q| (*p - *q).abs() < 0.5);
        for seg in cuts.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            if b - a < 0.5 {
                continue;
            }
            let mid = (a + b) / 2.0;
            let covering: Vec<&Cand> = group
                .iter()
                .filter(|c| c.a - 0.25 <= mid && mid <= c.b + 0.25)
                .collect();
            if covering.iter().any(|c| c.style == 1) {
                continue;
            }
            let Some(win) = covering.iter().max_by(|p, q| key_order(&p.key(), &q.key())) else {
                continue;
            };
            if win.w <= 0.0 || win.colour.a == 0.0 {
                continue;
            }
            let line = win.line;
            // Кромка стола лежит теперь на ТОЙ ЖЕ линии сетки, что и
            // кромки краевых ячеек, поэтому центрируются ОБЕ
            // (§17.6.2). Проверка: коробка стола начинается на
            // `линия − outer_win/2`, победившая полоса шириной `w`
            // занимает `линия ± w/2`, и при `w == outer_win` это ровно
            // `край … край + w` — прежний вжим внутрь давал ту же
            // полосу побайтно. При более узкой рамке стола полоса
            // стола и не рисуется: линию забирает более широкая ячейка.
            // Вжим по `lo_line` остаётся там, где пробы стола в группе
            // нет вовсе: у такой таблицы коробка совпадает с внешними
            // краями ячеек.
            let edge_dir = if outward.is_none() && (line - lo_line).abs() < 0.75 {
                Some(1)
            } else {
                None
            };
            let (lo, hi) = match edge_dir {
                Some(-1) => (line - win.w, line),
                Some(_) => (line, line + win.w),
                None => (line - win.w / 2.0, line + win.w / 2.0),
            };
            // Продление В УГЛЫ — только у сплошных (`style >= 9`):
            // пересечение иначе оставалось пустым квадратом, а
            // продление пунктирных рисовало лишние усы. Обе оси
            // тянутся на полуширину ПЕРПЕНДИКУЛЯРНОЙ кромки; кто из
            // них накроет угол, решает порядок по ключу (см. `segs`).
            let (a, b) = if win.style >= 9 {
                if vertical {
                    (a - half_at_h(a, line), b + half_at_h(b, line))
                } else {
                    (a - half_at(a, line), b + half_at(b, line))
                }
            } else {
                (a, b)
            };
            let rect = if vertical {
                Bounds {
                    origin: gpui::point(gpui::px(lo), gpui::px(a)),
                    size: gpui::size(gpui::px(hi - lo), gpui::px(b - a)),
                }
            } else {
                Bounds {
                    origin: gpui::point(gpui::px(a), gpui::px(lo)),
                    size: gpui::size(gpui::px(b - a), gpui::px(hi - lo)),
                }
            };
            segs.push((win.key(), vertical, rect, win.colour));
        }
        i = j;
    }
}

#[cfg(test)]
mod tests {
    use super::{Cand, Seg, draw_cands, key_order};
    use crate::style::values::value::Color;

    #[test]
    fn nan_widths_and_lines_keep_a_total_order() {
        // 64 candidates, every third with a NaN line or width: the sorts and
        // the winner choice must not panic (`sort_by` checks total order).
        let mut cands: Vec<Cand> = (0..64u32)
            .map(|i| {
                let nan = i % 3 == 0;
                Cand {
                    line: if nan {
                        f32::NAN
                    } else {
                        ((i * 37) % 101) as f32
                    },
                    a: 0.0,
                    b: 10.0,
                    w: if nan { f32::NAN } else { 1.0 + (i % 4) as f32 },
                    style: 9,
                    source: 0,
                    doc_ix: i,
                    colour: Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    outward: 0,
                }
            })
            .collect();
        let mut segs: Vec<Seg> = vec![];
        draw_cands(&mut cands, true, None, &|_, _| 0.5, &|_, _| 0.5, &mut segs);
        assert!(!segs.is_empty());
        segs.sort_by(|p, q| key_order(&p.0, &q.0));
        assert!(segs.windows(2).all(|w| key_order(&w[0].0, &w[1].0).is_le()));
    }
}
