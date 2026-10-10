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
mod edges;
pub use edges::edge_margins;
pub use edges::edge_sizes;

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
        "top-left-corner" => Place::Corner {
            top: true,
            left: true,
        },
        "top-right-corner" => Place::Corner {
            top: true,
            left: false,
        },
        "bottom-right-corner" => Place::Corner {
            top: false,
            left: false,
        },
        "bottom-left-corner" => Place::Corner {
            top: false,
            left: true,
        },
        "top-left" => Place::Edge { side: Top, at: 0 },
        "top-center" => Place::Edge { side: Top, at: 1 },
        "top-right" => Place::Edge { side: Top, at: 2 },
        "bottom-left" => Place::Edge {
            side: Bottom,
            at: 0,
        },
        "bottom-center" => Place::Edge {
            side: Bottom,
            at: 1,
        },
        "bottom-right" => Place::Edge {
            side: Bottom,
            at: 2,
        },
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
        Place::Corner {
            top: true,
            left: true,
        } => (0.0, 0.0, left, top),
        Place::Corner {
            top: true,
            left: false,
        } => (right_edge, 0.0, right, top),
        Place::Corner {
            top: false,
            left: false,
        } => (right_edge, bottom_edge, right, bottom),
        Place::Corner {
            top: false,
            left: true,
        } => (0.0, bottom_edge, left, bottom),
        Place::Edge {
            side: Side::Top, ..
        } => (m[3], 0.0, w - m[1] - m[3], top),
        Place::Edge {
            side: Side::Bottom, ..
        } => (m[3], bottom_edge, w - m[1] - m[3], bottom),
        Place::Edge {
            side: Side::Right, ..
        } => (right_edge, m[0], right, h - m[0] - m[2]),
        Place::Edge {
            side: Side::Left, ..
        } => (0.0, m[0], left, h - m[0] - m[2]),
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

#[cfg(test)]
mod tests;
