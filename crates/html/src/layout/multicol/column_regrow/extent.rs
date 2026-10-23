//! Продолжение измеренной копии: обрезка блочной оси гасит параллельный поток.

use crate::layout::fragment::types::{StackAxis, StackChild};
use gpui::{Pixels, Size, Window, point, px};

pub(super) fn measure(
    c: &mut StackChild,
    height: f32,
    content: Size<Pixels>,
    size: Size<Pixels>,
    axis: StackAxis,
    window: &mut Window,
) {
    // css-break-3 §fragmentation-model и css-overflow-3 §overflow-properties:
    // за обрезающим краем нет содержимого, создающего новые фрагментаинеры.
    // Blink fragmentation_utils.cc: kDisableFragmentation после конца clip box.
    if c.watched
        .as_ref()
        .is_some_and(|w| w.blocks().is_some_and(|b| b.clips_block))
    {
        c.over = height;
        return;
    }
    c.over = height.max(f32::from(if axis.is_vertical() {
        content.width
    } else {
        content.height
    }));
    if let Some(w) = &c.watched {
        let flow = w.flow.at(point(px(0.0), px(0.0)), size, axis, window);
        c.over = flow
            .floats
            .iter()
            .map(|g| g.bottom + g.margins[1])
            .fold(c.over, f32::max);
    }
}
