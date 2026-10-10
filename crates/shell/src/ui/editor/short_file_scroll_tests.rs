//! Регрессия INC-2026-0060: файл, целиком помещённый во вьюпорт, не
//! прокручивается ни колесом, ни кликом — строка 1 не уходит под строку пути.
//! Редактор собран как в `state/frame/editor.rs`: code editor без прокрутки за
//! последнюю строку (Monaco оригинала `scrollBeyondLastLine: false`).

use gpui::{
    AppContext as _, Context, Entity, Focusable as _, IntoElement, Modifiers, ParentElement as _,
    Render, ScrollDelta, ScrollWheelEvent, Styled as _, TestAppContext, TouchPhase,
    VisualTestContext, Window, div, point, px, size,
};
use gpui_component::input::{Input, InputState};

struct Host {
    input: Entity<InputState>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(Input::new(&self.input).h_full().appearance(false))
    }
}

fn editor(
    cx: &mut TestAppContext,
    lines: usize,
    beyond: bool,
) -> (Entity<InputState>, &mut VisualTestContext) {
    cx.update(gpui_component::init);
    let text: String = (1..=lines)
        .map(|i| format!("const v{i} = {i};\n"))
        .collect();
    let mut input = None;
    let w = cx.add_window(|window, cx| {
        let st = cx.new(|cx| {
            let mut st = InputState::new(window, cx)
                .code_editor("typescript")
                .scroll_beyond_last_line(beyond)
                .soft_wrap(false);
            st.set_value(text.clone(), window, cx);
            st
        });
        input = Some(st.clone());
        let host = cx.new(|_| Host { input: st });
        // Input читает `Root` окна (оверлеи, фокус), как в приложении
        gpui_component::Root::new(host, window, cx)
    });
    let cx = VisualTestContext::from_window(*w, cx).into_mut();
    cx.simulate_resize(size(px(600.), px(400.)));
    cx.run_until_parked();
    let st = input.unwrap();
    cx.update(|window, cx| window.focus(&st.read(cx).focus_handle(cx)));
    cx.run_until_parked();
    (st, cx)
}

fn offset_y(input: &Entity<InputState>, cx: &mut VisualTestContext) -> f32 {
    cx.update(|_, cx| f32::from(input.read(cx).scroll_handle.offset().y))
}

fn wheel_down(cx: &mut VisualTestContext, lines: f32) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(120.), px(100.)),
        delta: ScrollDelta::Lines(point(0., -lines)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
}

#[gpui::test]
fn wheel_does_not_scroll_a_file_that_fits(cx: &mut TestAppContext) {
    let (input, cx) = editor(cx, 9, false);
    wheel_down(cx, 5.);
    assert_eq!(offset_y(&input, cx), 0.0);
}

#[gpui::test]
fn upstream_overscroll_is_what_hid_line_one(cx: &mut TestAppContext) {
    // Контроль: с прокруткой за последнюю строку (upstream) тот же жест
    // уводит короткий файл вверх — ровно симптом карточки
    let (input, cx) = editor(cx, 9, true);
    wheel_down(cx, 5.);
    assert!(offset_y(&input, cx) < 0.0);
}

#[gpui::test]
fn long_file_still_scrolls_to_its_last_line(cx: &mut TestAppContext) {
    let (input, cx) = editor(cx, 200, false);
    wheel_down(cx, 1000.);
    let content = cx.update(|_, cx| f32::from(input.read(cx).scroll_content_height()));
    // Предел — высота текста минус вьюпорт (окно 400 минус собственные
    // отступы Input): после упора на экране остаётся ровно вьюпорт текста, а
    // не пустой хвост в полвьюпорта, как у upstream
    let off = offset_y(&input, cx);
    assert!(content > 400.0 && off < 0.0);
    let visible = content + off;
    assert!((300.0..=400.0).contains(&visible), "visible tail {visible}");
}

#[gpui::test]
fn click_on_visible_line_of_short_file_keeps_scroll(cx: &mut TestAppContext) {
    let (input, cx) = editor(cx, 9, false);
    for y in (10..260).step_by(10) {
        cx.simulate_click(point(px(120.), px(y as f32)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            offset_y(&input, cx),
            0.0,
            "click at y={y} scrolled the editor"
        );
    }
}
