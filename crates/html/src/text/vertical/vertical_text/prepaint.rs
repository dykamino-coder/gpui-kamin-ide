//! Prepaint for vertical_text; split out to keep the owning module within 250 lines.

use super::VerticalText;
use super::{VT_FRAME, VT_MEASURED};
use crate::text::vertical::VT_CCW;
use gpui::{App, Bounds, GlobalElementId, InspectorElementId, Pixels, Window};

impl VerticalText {
    pub(crate) fn prepaint_impl(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Ограничение доводится ЗДЕСЬ: на замере размеры коробки ещё
        // неизвестны, а на подготовке они уже решены раскладкой. Оси при этом
        // переставлены: то, что для родителя высота, для повёрнутого
        // содержимого — длина строки. Без этого шага вертикальный текст
        // мерился «по максимуму содержимого» и не переносился никогда.
        // ПРОБОВАЛИ И ОТКАТИЛИ: ограничивать длину строки высотой области
        // просмотра, когда родитель своей не задал (так велит CSS для
        // ортогональных потоков). Замерено: writing-modes 195 → 194,
        // `available-size-020/021` не чинятся, а `slr-alongside-vlr-floats`
        // ломается. Значит предел приходит откуда-то ещё.
        let along = bounds.size.height;
        let child = self.child.as_mut().unwrap();
        if along > gpui::px(0.) {
            let space = gpui::size(
                gpui::AvailableSpace::Definite(along),
                gpui::AvailableSpace::Definite(bounds.size.width),
            );
            let sized = child.layout_as_root_unrounded(space, window, cx);
            // Факт для следующего кадра: высота содержимого при решённой
            // длине строки — она и есть настоящая ширина повёрнутого блока.
            if let Some(k) = self.key
                && sized.height > gpui::px(0.)
            {
                VT_MEASURED.with(|c| {
                    let mut map = c.borrow_mut();
                    if map.len() >= 256 {
                        map.clear();
                    }
                    map.insert(k, sized.height);
                });
            }
        }
        // Щупу статической позиции нужна и СТОРОНА поворота: отображение
        // до-поворотной точки в экранную у `sideways-lr` зеркально (см. `vt_map`).
        let prev_ccw = VT_CCW.with(|c| c.replace(self.ccw));
        let prev = VT_FRAME.with(|c| c.replace(Some(bounds)));
        child.prepaint_at(bounds.origin, window, cx);
        VT_FRAME.with(|c| c.set(prev));
        VT_CCW.with(|c| c.set(prev_ccw));
    }
}
