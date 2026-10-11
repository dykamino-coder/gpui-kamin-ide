//! Типы потока фрагментов: полосы, дети, строки, ось стопки.
// owner: A

use crate::layout::float::shapes::FloatShape;
use gpui::AnyElement;
mod scope;
pub use scope::StackAxis;
pub use scope::StackScope;
pub use scope::axis_box;
pub use scope::in_stack;
pub use scope::set_outer_row;
pub use scope::take_outer_row;
pub(crate) use scope::{NOT_TOP, NotTop};
mod rows;
pub(crate) use rows::Frag;
pub use rows::Intrinsic;
pub use rows::Kid;
pub use rows::Par;
pub use rows::Rows;
mod stack_child;
pub use stack_child::Repeat;
pub use stack_child::RepeatGeom;
pub use stack_child::StackChild;

/// Полоса выреза: на строках, пересекающих [y0, y1), начало (или конец)
/// строки занято на `left`/`right` точек.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExBand {
    pub y0: f32,
    pub y1: f32,
    pub left: f32,
    pub right: f32,
}

/// Ребёнок потока: элемент и его известный размер.
pub struct FlowChild {
    pub el: AnyElement,
    pub w: f32,
    pub h: f32,
}

pub struct FlowRow {
    pub(crate) children: Vec<FlowChild>,
    pub(crate) shapes: std::sync::Arc<(Vec<FloatShape>, Vec<FloatShape>)>,
    /// Направление письма: rtl кладёт коробки от правого края.
    pub(crate) rtl: bool,
    /// `writing-mode: vertical-rl`: строки — колонки справа налево, поток в
    /// колонке — сверху вниз. Раскладка идёт в ТРАНСПОНИРОВАННОМ мире
    /// (инлайн-ось строкой), физика восстанавливается при укладке.
    pub(crate) vertical_rl: bool,
    /// Известный инлайн-размер содержащего блока (в вертикальном письме —
    /// его высота): запасной предел строк, когда замер его не даёт.
    pub(crate) inline_limit: Option<f32>,
    /// `vertical-lr`: колонки идут СЛЕВА направо (блок-старт — левый край).
    pub(crate) block_lr: bool,
    /// `sideways-lr`: инлайн-ось снизу вверх.
    pub(crate) inline_up: bool,
    /// Позиции детей, вычисленные замером (в точках от угла коробки).
    pub(crate) slots: std::cell::RefCell<Vec<(f32, f32)>>,
}

impl FlowRow {
    pub fn new(
        children: Vec<FlowChild>,
        shapes: std::sync::Arc<(Vec<FloatShape>, Vec<FloatShape>)>,
        rtl: bool,
    ) -> Self {
        FlowRow {
            children,
            shapes,
            rtl,
            vertical_rl: false,
            inline_limit: None,
            block_lr: false,
            inline_up: false,
            slots: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn vertical_rl(mut self) -> Self {
        self.vertical_rl = true;
        self
    }

    pub fn inline_up(mut self) -> Self {
        self.inline_up = true;
        self
    }

    pub fn block_lr(mut self) -> Self {
        self.block_lr = true;
        self
    }

    pub fn inline_limit(mut self, v: f32) -> Self {
        self.inline_limit = Some(v);
        self
    }

    /// Размер ребёнка в осях раскладки: в вертикальном письме инлайн-ось —
    /// физическая высота.
    pub(crate) fn tdims(&self, c: &FlowChild) -> (f32, f32) {
        if self.vertical_rl {
            (c.h, c.w)
        } else {
            (c.w, c.h)
        }
    }

    /// Вырез на полосе [y, y+h): точный экстент форм с обеих сторон.
    pub(crate) fn cut(&self, y: f32, h: f32) -> (f32, f32) {
        let l = self
            .shapes
            .0
            .iter()
            .map(|f| f.cut(y, y + h))
            .fold(0.0f32, f32::max);
        let r = self
            .shapes
            .1
            .iter()
            .map(|f| f.cut(y, y + h))
            .fold(0.0f32, f32::max);
        (l, r)
    }

    /// Нижний край всех форм: ниже него вырезов нет.
    pub(crate) fn shapes_bottom(&self) -> f32 {
        self.shapes
            .0
            .iter()
            .chain(self.shapes.1.iter())
            .map(|f| match *f {
                FloatShape::Band { top, h, .. } => top + h,
                FloatShape::Circle { top, cy, r, .. } => top + cy + r,
                FloatShape::Ellipse { top, cy, ry, .. } => top + cy + ry,
                FloatShape::Poly { top, ref pts } => {
                    top + pts.iter().map(|p| p.1).fold(0.0f32, f32::max)
                }
                FloatShape::Profile { top, ref ext } => top + ext.len() as f32,
                FloatShape::RoundedBox { top, ref shape, .. } => top + shape.bottom(),
            })
            .fold(0.0f32, f32::max)
    }

    /// Разложить детей в ширину `limit`; вернуть высоту и позиции.
    pub(crate) fn layout(&self, limit: f32) -> (f32, Vec<(f32, f32)>) {
        let mut slots = Vec::with_capacity(self.children.len());
        let mut y = 0.0f32;
        let mut x = 0.0f32;
        let mut line_h = 0.0f32;
        let mut cut = self.cut(0.0, 1.0);
        for c in &self.children {
            let (cw, ch) = self.tdims(c);
            let avail = (limit - cut.0 - cut.1).max(0.0);
            // ЗАМЕРЕНО И ОТКАЧЕНО: гасить перенос при `white-space: nowrap`
            // (поля `nowrap` у ряда не было вовсе). Полный свод CSS3: 0 и 0.
            // Заявленные пары в своде не нашлись под названными именами —
            // полосный ряд включается только при обтекании, а тесты гибкой
            // раскладки флоатов не содержат.
            // Не влезает — новая строка; коробка шире строки стоит одна.
            if x + cw > avail + 0.01 && x > 0.0 {
                y += line_h;
                x = 0.0;
                line_h = 0.0;
                cut = self.cut(y, ch.max(1.0));
            } else if x == 0.0 {
                cut = self.cut(y, ch.max(1.0));
            }
            // Коробка не помещается даже в начале строки — строка съезжает
            // ниже, пока вырез не отпустит (float-retry-push): плавающий
            // блок толкает СЛИШКОМ ШИРОКОЕ содержимое под себя.
            if x == 0.0 && cw > (limit - cut.0 - cut.1).max(0.0) + 0.01 {
                let bottom = self.shapes_bottom();
                while y < bottom {
                    y += 1.0;
                    cut = self.cut(y, ch.max(1.0));
                    if cw <= (limit - cut.0 - cut.1).max(0.0) + 0.01 {
                        break;
                    }
                }
            }
            // Вырез мог смениться выше по строке — пересчитать после переноса.
            let (sx, sy) = if self.rtl {
                (limit - cut.1 - x - cw, y)
            } else {
                (cut.0 + x, y)
            };
            slots.push((sx, sy));
            x += cw;
            line_h = line_h.max(ch);
        }
        (y + line_h, slots)
    }
}
