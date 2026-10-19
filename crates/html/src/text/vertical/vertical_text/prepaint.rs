//! Resolve the rotated atomic subtree and record its document-owned measure.

use super::VT_FRAME;
use super::VerticalText;
use crate::text::vertical::{VT_CCW, VT_PHASE};
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
        // ПРОБОВАЛИ И ОТКАТИЛИ: ограничивать длину строки высотой области
        // просмотра, когда родитель своей не задал (так велит CSS для
        // ортогональных потоков). Замерено: writing-modes 195 → 194,
        // `available-size-020/021` не чинятся, а `slr-alongside-vlr-floats`
        // ломается. Значит предел приходит откуда-то ещё.
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
        // до-поворотной точки в экранную у `sideways-lr` зеркально (см. `vt_map`).
        let (frame, phase) = self.turn_frame(bounds, window);
        let prev_ccw = VT_CCW.with(|c| c.replace(self.ccw));
        let prev = VT_FRAME.with(|c| c.replace(Some(frame)));
        let prev_phase = VT_PHASE.with(|c| c.replace(phase));
        let flat = gpui::point(bounds.origin.x, bounds.origin.y + phase);
        self.child.prepaint_at(flat, window, cx);
        VT_PHASE.with(|c| c.set(prev_phase));
        VT_FRAME.with(|c| c.set(prev));
        VT_CCW.with(|c| c.set(prev_ccw));
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
        if !delta.is_finite() || delta.abs() > 0.5 {
            return (bounds, Pixels::ZERO);
        }
        let mut frame = bounds;
        frame.size.width += px(delta);
        (frame, px(-delta))
    }
}
