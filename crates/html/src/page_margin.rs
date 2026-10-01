//! Раскладка марджин-боксов листа (css-page-3 §margin-dimension) — по Blink
//! `PageContainerLayoutAlgorithm` (`page_container_layout_algorithm.cc`):
//! `LayoutAllMarginBoxes` (:311-384) — прямоугольники углов и сторон,
//! `CalculateEdgeMarginBoxSizes` (:587-691) и `ResolveTwoEdgeMarginBoxLengths`
//! (:693-779) — размеры по главной оси стороны, `ResolveMarginsForPageMarginBox`
//! (:59-93) — поля и пере-определение по поперечной. `min-width`/`max-width`
//! Blink не применяет (TODO crbug 40341678) — и здесь тоже.
//!
//! Чистые функции: мерить содержимое и раскладывать элементы — дело
//! `flow::PageStack`, здесь только арифметика.

/// Место коробки на листе.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// Угол: пересечение двух полей листа.
    Corner { top: bool, left: bool },
    /// Сторона и номер на ней: 0 — начало (левая/верхняя), 1 — середина,
    /// 2 — конец (правая/нижняя).
    Edge { side: Side, at: usize },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Side {
    /// Главная ось стороны горизонтальна (верх и низ).
    pub fn horizontal(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }
}

/// Место коробки по имени (`css::MARGIN_BOXES`).
pub fn place(name: &str) -> Option<Place> {
    use Side::*;
    Some(match name {
        "top-left-corner" => Place::Corner { top: true, left: true },
        "top-right-corner" => Place::Corner { top: true, left: false },
        "bottom-right-corner" => Place::Corner { top: false, left: false },
        "bottom-left-corner" => Place::Corner { top: false, left: true },
        "top-left" => Place::Edge { side: Top, at: 0 },
        "top-center" => Place::Edge { side: Top, at: 1 },
        "top-right" => Place::Edge { side: Top, at: 2 },
        "bottom-left" => Place::Edge { side: Bottom, at: 0 },
        "bottom-center" => Place::Edge { side: Bottom, at: 1 },
        "bottom-right" => Place::Edge { side: Bottom, at: 2 },
        "left-top" => Place::Edge { side: Left, at: 0 },
        "left-middle" => Place::Edge { side: Left, at: 1 },
        "left-bottom" => Place::Edge { side: Left, at: 2 },
        "right-top" => Place::Edge { side: Right, at: 0 },
        "right-middle" => Place::Edge { side: Right, at: 1 },
        "right-bottom" => Place::Edge { side: Right, at: 2 },
        _ => return None,
    })
}

/// Умолчания `text-align` и `vertical-align` коробки (css-page-3
/// §margin-text-alignment, Table 2).
pub fn defaults(name: &str) -> (&'static str, &'static str) {
    match name {
        "top-left-corner" | "bottom-left-corner" => ("right", "middle"),
        "top-right-corner" | "bottom-right-corner" => ("left", "middle"),
        "top-left" | "bottom-left" => ("left", "middle"),
        "top-center" | "bottom-center" => ("center", "middle"),
        "top-right" | "bottom-right" => ("right", "middle"),
        "left-top" | "right-top" => ("center", "top"),
        "left-bottom" | "right-bottom" => ("center", "bottom"),
        _ => ("center", "middle"),
    }
}

/// Прямоугольник `(x, y, w, h)` в точках листа.
pub type Rect = (f32, f32, f32, f32);

/// Содержащий блок коробки (Blink `LayoutAllMarginBoxes`): углы — пересечение
/// полей, стороны — поле между углами. Отрицательные поля — ноль.
pub fn containing_block(p: Place, size: (f32, f32), m: [f32; 4]) -> Rect {
    let (w, h) = size;
    let (top, right, bottom, left) = (m[0].max(0.0), m[1].max(0.0), m[2].max(0.0), m[3].max(0.0));
    let right_edge = w - m[1];
    let bottom_edge = h - m[2];
    match p {
        Place::Corner { top: true, left: true } => (0.0, 0.0, left, top),
        Place::Corner { top: true, left: false } => (right_edge, 0.0, right, top),
        Place::Corner { top: false, left: false } => (right_edge, bottom_edge, right, bottom),
        Place::Corner { top: false, left: true } => (0.0, bottom_edge, left, bottom),
        Place::Edge { side: Side::Top, .. } => (m[3], 0.0, w - m[1] - m[3], top),
        Place::Edge { side: Side::Bottom, .. } => (m[3], bottom_edge, w - m[1] - m[3], bottom),
        Place::Edge { side: Side::Right, .. } => (right_edge, m[0], right, h - m[0] - m[2]),
        Place::Edge { side: Side::Left, .. } => (0.0, m[0], left, h - m[0] - m[2]),
    }
}

/// Предпочтительный размер коробки по главной оси (Blink `PreferredSizeInfo`):
/// min/max — border box, `margins` — сумма полей по оси (`auto` = 0), `auto`
/// — размер по оси не задан.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pref {
    pub min: f32,
    pub max: f32,
    pub margins: f32,
    pub auto: bool,
}

impl Pref {
    fn min_len(&self) -> f32 {
        self.min + self.margins
    }
    fn max_len(&self) -> f32 {
        self.max + self.margins
    }
    fn doubled(&self) -> Pref {
        Pref {
            min: self.min * 2.0,
            max: self.max * 2.0,
            margins: self.margins * 2.0,
            auto: self.auto,
        }
    }
}

/// Две коробки из трёх (`[первая, не решаемая, вторая]`) делят доступную
/// длину — Blink `ResolveTwoEdgeMarginBoxLengths`. Итог — длины ПО ПОЛЯМ.
fn resolve_two(p: [Pref; 3], avail: f32) -> (f32, f32) {
    let mut flex_avail = avail;
    let (mut tmin, mut tmax) = (0.0f32, 0.0f32);
    for x in &p {
        if x.auto {
            tmin += x.min_len();
            tmax += x.max_len();
        } else {
            flex_avail -= x.max_len();
        }
    }
    let (space, unflexed, factors): (f32, [f32; 3], [f32; 3]) = if flex_avail > tmax {
        let u = [p[0].max_len(), p[1].max_len(), p[2].max_len()];
        (flex_avail - tmax, u, u)
    } else {
        let u = [p[0].min_len(), p[1].min_len(), p[2].min_len()];
        let space = flex_avail - tmin;
        let f = if space > 0.0 {
            [
                p[0].max_len() - p[0].min_len(),
                p[1].max_len() - p[1].min_len(),
                p[2].max_len() - p[2].min_len(),
            ]
        } else {
            u
        };
        (space, u, f)
    };
    let mut first = unflexed[0];
    let mut second = unflexed[2];
    if p[0].auto {
        if p[2].auto {
            let total = factors[0] + factors[2];
            if total > 0.0 {
                first += space * factors[0] / total;
            }
        } else {
            first = avail - second;
        }
    }
    if p[2].auto {
        second = avail - first;
    }
    (first, second)
}

/// Длины трёх коробок стороны по главной оси (border box; `None` — коробки
/// нет). Blink `CalculateEdgeMarginBoxSizes`.
pub fn edge_sizes(prefs: [Option<Pref>; 3], avail: f32) -> [f32; 3] {
    let mut p: [Pref; 3] = [Pref::default(); 3];
    let mut out = [0.0f32; 3];
    let mut auto_max = 0.0f32;
    let mut any_auto = false;
    for i in 0..3 {
        if let Some(x) = prefs[i] {
            p[i] = x;
            out[i] = x.max_len();
            if x.auto {
                any_auto = true;
                auto_max += x.max_len();
            }
        }
    }
    // Ни у одной `auto`-коробки нет содержимого — место делится поровну.
    if any_auto && auto_max == 0.0 {
        for x in p.iter_mut().filter(|x| x.auto) {
            *x = Pref {
                min: 1.0,
                max: 1.0,
                margins: 0.0,
                auto: true,
            };
        }
    }
    if prefs[1].is_some() {
        if p[1].auto {
            // Воображаемая коробка AC — удвоенная начальная, затем удвоенная
            // конечная; центр — меньший из двух исходов (центровка B).
            let (c1, _) = resolve_two([p[1], Pref::default(), p[0].doubled()], avail);
            let (c2, _) = resolve_two([p[1], Pref::default(), p[2].doubled()], avail);
            out[1] = c1.min(c2);
        }
        let side = avail - out[1];
        if p[0].auto {
            out[0] = side / 2.0;
        }
        if p[2].auto {
            out[2] = side - side / 2.0;
        }
    } else {
        let (a, c) = resolve_two(p, avail);
        out[0] = a;
        out[2] = c;
    }
    for i in 0..3 {
        out[i] = (out[i] - p[i].margins).max(0.0);
    }
    out
}

/// Поля коробки по оси, прилегающей к краю бумаги (Blink
/// `ResolveMarginsForPageMarginBox`): `auto` делят остаток, пере-определение
/// снимается полем, смотрящим ОТ центра листа (`at_start` — коробка у
/// начального края оси: верхнего или левого).
pub fn edge_margins(
    start: Option<f32>,
    end: Option<f32>,
    border_box: f32,
    avail: f32,
    at_start: bool,
) -> (f32, f32) {
    let (mut s, mut e) = (start.unwrap_or(0.0), end.unwrap_or(0.0));
    let extra = (avail - border_box - s - e).max(0.0);
    match (start, end) {
        (None, None) => {
            s += extra / 2.0;
            e += extra - extra / 2.0;
        }
        (None, Some(_)) => s += extra,
        (Some(_), None) => e += extra,
        _ => {}
    }
    let gap = avail - (border_box + s + e);
    if gap != 0.0 {
        if at_start {
            s += gap;
        } else {
            e += gap;
        }
    }
    (s, e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auto(max: f32) -> Option<Pref> {
        Some(Pref {
            min: max,
            max,
            margins: 0.0,
            auto: true,
        })
    }

    #[test]
    fn three_equal_boxes_share_width() {
        // `alignment-001`: по букве в каждой коробке — три равные трети.
        let s = edge_sizes([auto(8.0), auto(8.0), auto(8.0)], 450.0);
        assert!((s[0] - 150.0).abs() < 0.01 && (s[1] - 150.0).abs() < 0.01);
        assert!((s[2] - 150.0).abs() < 0.01);
    }

    #[test]
    fn fixed_center_rest_split() {
        let fixed = Some(Pref {
            min: 100.0,
            max: 100.0,
            margins: 0.0,
            auto: false,
        });
        let s = edge_sizes([auto(10.0), fixed, auto(30.0)], 300.0);
        assert_eq!(s, [100.0, 100.0, 100.0]);
    }

    #[test]
    fn overconstrained_moves_away_from_center() {
        // Верхняя коробка 50 при поле 100: остаток уходит в верхнее поле.
        assert_eq!(edge_margins(Some(0.0), Some(0.0), 50.0, 100.0, true), (50.0, 0.0));
        assert_eq!(edge_margins(None, None, 50.0, 100.0, true), (25.0, 25.0));
    }
}
