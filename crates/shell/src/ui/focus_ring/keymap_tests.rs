//! Клавиатурный маршрут startup → RootView → rename и tab-стопы (BR-29).

use super::*;
use crate::host_link::ShellEvent;
use crate::state::model::RootView;
use gpui::{Context, Entity, IntoElement, Keystroke, Render, TestAppContext, VisualTestContext};
use gpui_component::input::InputState;
use smol::channel::Receiver;

static SERIAL: Mutex<()> = Mutex::new(());
const FIRST: &str = "srow:keymap-first";
const SECOND: &str = "srow:keymap-second";

struct Keys {
    root: Entity<RootView>,
    webview: bool,
}

impl Render for Keys {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        begin_frame(window);
        if flush(cx) {
            cx.notify();
        }
        self.root.update(cx, |root, cx| {
            let p = kamin_theme::ThemeKind::Dark.palette();
            let mut content = div()
                .size_full()
                .track_focus(&root.focus_handle)
                .key_context(if self.webview { "Root webview" } else { "Root" });
            if let Some(input) = &root.rename_input {
                content = content.child(crate::ui::sessions::rename_row::rename_row(
                    input,
                    div().id("rename-row"),
                    false,
                    false,
                    false,
                    gpui::Empty.into_any_element(),
                    &root.tx,
                    p,
                ));
            } else {
                for id in [FIRST, SECOND] {
                    content = content.child(
                        div()
                            .id(id)
                            .w(px(100.))
                            .h(px(24.))
                            .focus_ring(id, 2., gpui::red().into())
                            .child(id),
                    );
                }
            }
            root.bind_actions(content, cx)
        })
    }
}

fn fixture(
    cx: &mut TestAppContext,
) -> (
    Entity<Keys>,
    Entity<RootView>,
    Receiver<ShellEvent>,
    &mut VisualTestContext,
) {
    HANDLES.lock().unwrap().clear();
    PENDING.lock().unwrap().clear();
    *FRAME.lock().unwrap() = (false, None);
    cx.update(crate::startup::configure);
    let (tx, rx) = smol::channel::unbounded();
    let (keys, visual) = cx.add_window_view(|window, cx| {
        let root = cx.new(|cx| RootView::initial_state(tx, cx));
        window.focus(&root.read(cx).focus_handle.clone(), cx);
        Keys {
            root,
            webview: false,
        }
    });
    let root = keys.read_with(visual, |keys, _| keys.root.clone());
    // Первый кадр запрашивает хэндлы, второй чеканит, третий рисует tab-стопы.
    for _ in 0..3 {
        draw(visual);
    }
    (keys, root, rx, visual)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn press(key: &str, cx: &mut VisualTestContext) -> bool {
    let handled =
        cx.update(|window, cx| window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx));
    draw(cx);
    handled
}

fn rename(root: &Entity<RootView>, cx: &mut VisualTestContext) {
    root.update_in(cx, |root, window, cx| {
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value("Renamed session", window, cx);
            input
        });
        use gpui::Focusable as _;
        window.focus(&input.read(cx).focus_handle(cx), cx);
        root.renaming_session = Some("session-under-edit".into());
        root.rename_input = Some(input);
        cx.notify();
    });
    draw(cx);
}

fn commits(rx: &Receiver<ShellEvent>) -> usize {
    std::iter::from_fn(|| rx.try_recv().ok())
        .filter(|event| matches!(event, ShellEvent::CommitRename))
        .count()
}

#[gpui::test]
fn enter_commits_the_focused_rename_input_once_and_escape_still_cancels(cx: &mut TestAppContext) {
    let _guard = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    let (_, root, rx, cx) = fixture(cx);
    rename(&root, cx);
    assert!(press("enter", cx));
    assert_eq!(commits(&rx), 1, "Enter must enqueue one CommitRename");
    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.rename_input.as_ref().unwrap().read(cx).value(),
            "Renamed session"
        );
    });
    assert!(press("escape", cx));
    root.read_with(cx, |root, _| {
        assert!(root.renaming_session.is_none());
        assert!(root.rename_input.is_none());
    });
    assert_eq!(commits(&rx), 0, "Escape must not commit");
}

#[gpui::test]
fn tab_and_shift_tab_move_the_visible_focus_ring_and_respect_webview(cx: &mut TestAppContext) {
    let _guard = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    let (keys, _, _, cx) = fixture(cx);
    cx.update(|window, cx| assert!(focus_id(FIRST, window, cx)));
    draw(cx);
    assert_eq!(focused_id().as_deref(), Some(FIRST));
    assert!(press("tab", cx), "Tab must reach focus traversal");
    assert!(debug_state().0);
    assert_eq!(focused_id().as_deref(), Some(SECOND));
    assert!(press("shift-tab", cx));
    assert_eq!(focused_id().as_deref(), Some(FIRST));
    // Проверяем wrap и каждое нажатие, а не только итог чётных 80 шагов.
    for index in 0..80 {
        assert!(press("tab", cx));
        let expected = if index % 2 == 0 { SECOND } else { FIRST };
        assert_eq!(focused_id().as_deref(), Some(expected));
    }
    keys.update(cx, |keys, cx| {
        keys.webview = true;
        cx.notify();
    });
    draw(cx);
    assert!(!press("tab", cx), "Chromium retains its own Tab path");
    assert!(!press("shift-tab", cx));
    assert_eq!(focused_id().as_deref(), Some(FIRST));
}
