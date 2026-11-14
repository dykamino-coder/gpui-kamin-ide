//! Шаг документа не копит память между кадрами: после каждого кадра арена
//! элементов пуста, отложенных состояний нет, а кэш строк кадра — тот же, что
//! у кадров без шага, и на кадрах одной ширины одного размера. Кадр
//! собирается внутри `draw` только после отказа заместителя записи
//! (`layout::reuse`).

use super::l0_tests::{PAGE, builds};
use super::tests::{View, frame, open, open_without_step};
use gpui::{Entity, TestAppContext, VisualTestContext};

const FRAMES: usize = 16;

/// What each of `FRAMES` frames of alternating widths leaves: element arena
/// allocations, frame line layouts, set aside element states.
fn footprints(view: &Entity<View>, cx: &mut VisualTestContext) -> Vec<(usize, usize, usize)> {
    frame(cx);
    (0..FRAMES)
        .map(|i| {
            let width = if i % 2 == 0 { 500. } else { 400. };
            cx.update(|_, app| view.update(app, |v, _| v.viewport = (width, 300.)));
            frame(cx);
            cx.update(|window, app| window.layout_footprint(app))
        })
        .collect()
}

#[gpui::test]
fn steps_leave_no_arena_lines_or_states_growing_between_frames(cx: &mut TestAppContext) {
    let base = {
        let (view, cx) = open_without_step(cx, PAGE);
        footprints(&view, cx)
    };
    let (view, cx) = open(cx, PAGE);
    let (built, inside) = builds(&view, cx);
    let refused = cx.update(|_, app| view.read(app).doc.reused_blocks().2);
    let seen = footprints(&view, cx);
    let (built_after, inside_after) = builds(&view, cx);
    let refused_after = cx.update(|_, app| view.read(app).doc.reused_blocks().2);
    assert!(
        built_after >= built + FRAMES as u64,
        "the step did not build every frame: {built} -> {built_after}"
    );
    // A frame builds inside the draw only after its prepaint refused a
    // placeholder of the step's tree (the screen check needs the frame).
    assert_eq!(
        inside_after - inside,
        refused_after - refused,
        "frames were built inside draw without a refused placeholder"
    );
    assert_eq!(seen, base, "the step changed what frames leave behind");
    for (arena, _, states) in &seen {
        assert_eq!((*arena, *states), (0, 0), "a frame left elements or states: {seen:?}");
    }
    // After the first frames at each width, each width keeps its line count.
    for i in FRAMES / 2..FRAMES {
        assert_eq!(seen[i].1, seen[i - 2].1, "the line cache grew: {seen:?}");
    }
}
