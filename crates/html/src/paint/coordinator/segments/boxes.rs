//! Собственные декорации узла по фрагментам его допуска, через общий painter коробки.

use super::*;

pub(crate) fn decorations(id: u64, window: &Window) -> Rc<[Decoration]> {
    let key = capture_key(window);
    window
        .render_context::<Source>()
        .and_then(|source| source.boxes(id, key, shift::current(window)))
        .unwrap_or_else(|| Rc::from([]))
}

pub(crate) fn decoration(
    geometry: Decoration,
    window: &mut Window,
    paint: impl FnOnce(&mut Window),
) {
    let result = window.with_paint_segment_slices(Rc::from([geometry.slice]), paint);
    if let Err(error) = result
        && let Some(source) = window.render_context::<Source>()
    {
        source.fail(error);
    }
}

pub(crate) fn box_painter(id: u64) -> gpui::BoxPainter {
    gpui::BoxPainter(Rc::new(move |style, _, window, cx| {
        paint_box(id, style, window, cx)
    }))
}

fn paint_box(id: u64, style: &gpui::Style, window: &mut Window, cx: &mut App) -> bool {
    let Some(source) = window.render_context::<Source>() else {
        return false;
    };
    let key = capture_key(window);
    // Только узлы с явным вводом заменяют свою краску; служебные native-узлы
    // продолжают общий painter без CSS-эвристик в GPUI.
    let boxes = source.boxes(id, key, shift::current(window));
    let Some(boxes) = boxes else {
        return false;
    };
    for geometry in boxes.iter() {
        decoration(*geometry, window, |window| {
            style.paint_box(geometry.bounds, window, cx)
        });
    }
    true
}
