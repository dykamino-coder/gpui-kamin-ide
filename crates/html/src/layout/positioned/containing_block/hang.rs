//! Висячий начальный край строки (InlineStartHang) для позднего размещения.

use crate::hooks::Wrap;
use gpui::{AnyElement, App, Bounds, Pixels, Window, px};

/// Коробка, повешенная на точку своим строчным НАЧАЛОМ.
///
/// Текстовый путь абзаца кладёт кусок вне потока ЛЕВЫМ верхним углом на точку
/// (`lines.rs: point_of` -> `prepaint_at`). При `direction: rtl` строчное
/// начало — ПРАВЫЙ край (CSS 2.1 §10.3.7: «otherwise, set 'right' to the
/// static position»; css-position-3 §staticpos-rect: прямоугольник
/// статической позиции «positioned at its inline-start static position»),
/// поэтому коробку надо сдвинуть назад ровно на свою ширину. Обёртка
/// возвращает раскладке `layout_id` ребёнка, то есть в потоке ничего не
/// добавляет и ничего не меряет заново.
///
/// Blink: `geometry/static_position.h:86` даёт `kInlineEnd` при `!IsLtr()`,
/// `absolute_utils.cc:27-37 GetStaticPositionInsetBias` переводит его в
/// `InsetBias::kEnd`.
///
/// Поля в сдвиг не входят — ровно как и у ltr-ветки, где на точку садится край
/// РАМОЧНОЙ коробки, а не отбивочной. Обе стороны считаются одинаково, и
/// разница проявилась бы только у абсолюта с ненулевым боковым полем.
pub struct InlineStartHang {
    pub(crate) child: AnyElement,
}

impl InlineStartHang {
    pub fn new(child: AnyElement) -> Self {
        Self { child }
    }
}

impl Wrap for InlineStartHang {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let child = &mut self.child;
        let shift = gpui::point(px(0.0) - bounds.size.width, px(0.0));
        window.with_exact_element_offset(shift, |window| child.prepaint(window, cx));
    }
}
