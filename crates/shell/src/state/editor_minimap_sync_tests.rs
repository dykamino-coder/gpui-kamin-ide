//! Регрессия INC-2026-0059: зеркало минимапы принадлежит табу и догоняет его
//! буфер, а не остаётся текстом последнего открытого файла.

use super::{new_mirror, sync_mirror};
use crate::state::editor_tab::EditorTab;
use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};
use gpui_component::input::InputState;

fn tab(path: &str, text: &str, cx: &mut VisualTestContext) -> EditorTab {
    let (input, minimap) = cx.update(|window, cx| {
        let input = cx.new(|cx| {
            let mut st = InputState::new(window, cx).code_editor("rust");
            st.set_value(text.to_string(), window, cx);
            st
        });
        let minimap = new_mirror("rust", text.to_string(), window, cx);
        (input, minimap)
    });
    EditorTab {
        path: path.into(),
        input,
        minimap,
        minimap_stale: false,
        dirty: false,
        eol: "LF",
        last_used: std::time::Instant::now(),
        pinned: false,
        _sub: gpui::Subscription::new(|| {}),
    }
}

fn text(e: &Entity<InputState>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| e.read(cx).value().to_string())
}

struct Blank;
impl gpui::Render for Blank {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::Empty
    }
}

fn window(cx: &mut TestAppContext) -> &mut VisualTestContext {
    cx.update(gpui_component::init);
    let w = cx.add_window(|_, _| Blank);
    VisualTestContext::from_window(*w, cx).into_mut()
}

#[gpui::test]
fn each_tab_keeps_its_own_mirror(cx: &mut TestAppContext) {
    let cx = window(cx);
    let a = tab("a.rs", "fn a() {}\n", cx);
    let b = tab("b.rs", "fn b() {}\n", cx);
    // Открытие b не трогает зеркало a: раньше зеркало было одно на редактор
    assert_eq!(text(&a.minimap, cx), "fn a() {}\n");
    assert_eq!(text(&b.minimap, cx), "fn b() {}\n");
}

#[gpui::test]
fn stale_mirror_catches_up_with_edits(cx: &mut TestAppContext) {
    let cx = window(cx);
    let mut t = tab("a.rs", "one\n", cx);
    cx.update(|window, cx| {
        t.input.update(cx, |st, cx| {
            st.set_value("one\ntwo\n".to_string(), window, cx)
        });
    });
    // Без метки синхронизации нет: копия текста только по событию Change
    cx.update(|window, cx| sync_mirror(&mut t, window, cx));
    assert_eq!(text(&t.minimap, cx), "one\n");
    t.minimap_stale = true;
    cx.update(|window, cx| sync_mirror(&mut t, window, cx));
    assert_eq!(text(&t.minimap, cx), "one\ntwo\n");
    assert!(!t.minimap_stale);
}
