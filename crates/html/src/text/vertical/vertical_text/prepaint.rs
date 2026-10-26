//! Фаза `prepaint` элемента `VerticalText`: решённая раскладкой высота коробки становится
//! длиной строки содержимого (перенос вертикального текста), высота содержимого при ней
//! запоминается в состоянии документа для следующего кадра (`vertical_measures`), а на время
//! подготовки потомков выставляется рамка поворота (`TurnFrame`: сторона и фаза); рамка
//! привязана к неокруглённому block-end краю (css-writing-modes-4 §7.1; Blink
//! `WritingModeConverter::SlowConvert`).

use super::VerticalText;
use crate::text::vertical::TurnFrame;
use gpui::{App, Bounds, Pixels, Window, px};

impl VerticalText {
    pub(crate) fn prepaint_impl(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Ограничение доводится ЗДЕСЬ: на замере размеры коробки ещё
        // неизвестны, а на подготовке они уже решены раскладкой. Оси при этом
        // переставлены: то, что для родителя высота, для повёрнутого
        // содержимого — длина строки. Без этого шага вертикальный текст
        // мерился «по максимуму содержимого» и не переносился никогда.
        // Не ограничивать длину строки высотой области просмотра, когда
        // родитель своей не задал (как велит CSS для ортогональных потоков):
        // `available-size-020/021` это не чинит, а `slr-alongside-vlr-floats`
        // ломает — предел приходит из другого места.
        let along = bounds.size.height;
        let child = &mut self.child;
        if along > gpui::px(0.) {
            let space = gpui::size(
                gpui::AvailableSpace::Definite(along),
                gpui::AvailableSpace::Definite(bounds.size.width),
            );
            let sized = child.layout_as_root_unrounded(space, window, cx);
            // Факт для следующего кадра: высота содержимого при решённой
            // длине строки — она и есть настоящая ширина повёрнутого блока.
            if let Some(k) = self.key
                && let Some(values) = &self.values
                && sized.height > gpui::px(0.)
            {
                values.vcx().doc.vertical_measures.record(k, sized.height);
            }
        }
        // Щупу статической позиции нужна и СТОРОНА поворота: отображение
        // до-поворотной точки в экранную у `sideways-lr` зеркально (см.
        // `VerticalPaint::map`).
        let (frame, phase) = self.turn_frame(bounds, window);
        let previous = self.paint_context.replace(Some(TurnFrame {
            bounds: frame,
            ccw: self.ccw,
            phase,
        }));
        let flat = gpui::point(bounds.origin.x, bounds.origin.y + phase);
        self.child.prepaint_at(flat, window, cx);
        self.paint_context.replace(previous);
        self.turn = Some((frame.size.width, phase));
    }

    /// Clockwise frame anchored at the UNROUNDED block-end edge.
    ///
    /// In `vertical-rl`/`vertical-lr` the block axis is physical x, and
    /// css-writing-modes-4 §7.1 places lines from the block-start edge of
    /// the box's own (unrounded) extent — Blink converts logical offsets
    /// with the LayoutUnit container width (`WritingModeConverter::
    /// SlowConvert`) and snaps only the painted physical rectangles. The
    /// flat content is turned about the right edge, so the turn uses the
    /// unrounded width, and the flat origin is shifted by the phase that
    /// makes `right + flat_y` integral: edges rounded before the turn
    /// (atomic inline roots) then round exactly like physical edges.
    fn turn_frame(&self, bounds: Bounds<Pixels>, window: &mut Window) -> (Bounds<Pixels>, Pixels) {
        let Some(id) = self.layout_id.filter(|_| !self.ccw) else {
            return (bounds, Pixels::ZERO);
        };
        let scale = window.scale_factor();
        let right = f32::from(
            window.layout_origin_unrounded(id).x + window.layout_size_unrounded(id).width,
        );
        let delta = right - (right * scale).round() / scale;
        if !delta.is_finite() || delta.abs() > crate::tolerance::TEXT_ORIGIN_SNAP_EPS {
            return (bounds, Pixels::ZERO);
        }
        let mut frame = bounds;
        frame.size.width += px(delta);
        (frame, px(-delta))
    }
}
