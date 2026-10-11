//! Висячий начальный край строки (InlineStartHang) для позднего размещения.

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, px,
};

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
    pub(crate) child: Option<AnyElement>,
}

impl InlineStartHang {
    pub fn new(child: AnyElement) -> Self {
        Self { child: Some(child) }
    }
}

impl Element for InlineStartHang {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let child = self.child.as_mut().unwrap();
        let shift = gpui::point(px(0.0) - bounds.size.width, px(0.0));
        window.with_exact_element_offset(shift, |window| child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.as_mut().unwrap().paint(window, cx);
    }
}

impl IntoElement for InlineStartHang {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}
