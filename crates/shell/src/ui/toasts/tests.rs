//! NEW-NOTIFY-ONE-LINE: длинный тост переносится в границах стопки (360),
//! высота растёт по строкам до предела, короткий остаётся узким у правого края.

use super::*;
use crate::host_link::ShellEvent;
use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, VisualTestContext, Window,
};

const SLOT: f32 = 360.0;

struct Stack {
    items: Vec<Toast>,
    tx: Sender<ShellEvent>,
}

impl Render for Stack {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let p = kamin_theme::ThemeKind::Dark.palette();
        // Как слот в overlay/layers/passive.rs: ширина 360, карты прижаты вправо.
        div()
            .debug_selector(|| "slot".to_string())
            .w(px(SLOT))
            .flex()
            .flex_col()
            .items_end()
            .child(toasts(&self.items, &HashMap::new(), &self.tx, p))
    }
}

fn toast(id: &str, message: &str, actions: &[&str]) -> Toast {
    Toast {
        id: id.to_string(),
        severity: "error".to_string(),
        title: Some("Bridge".to_string()),
        message: message.to_string(),
        actions: actions.iter().map(|a| a.to_string()).collect(),
        sticky: true,
        duration_ms: None,
    }
}

fn layout(cx: &mut TestAppContext, items: Vec<Toast>) -> (Vec<Bounds<Pixels>>, Bounds<Pixels>) {
    cx.update(crate::startup::configure);
    let (tx, _rx) = smol::channel::unbounded();
    let ids: Vec<String> = items.iter().map(|t| t.id.clone()).collect();
    let (_, cx): (_, &mut VisualTestContext) = cx.add_window_view(|_, _| Stack { items, tx });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    // The slide-in (mr < 0 for TOAST_OUT_MS, wall clock) must be over before
    // the edges are measured.
    std::thread::sleep(Duration::from_millis(TOAST_OUT_MS + 60));
    for _ in 0..2 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    // debug_bounds wants a 'static selector: leaked once per test card.
    let cards = ids
        .iter()
        .map(|id| {
            let selector: &'static str = Box::leak(format!("toast-card-{id}").into_boxed_str());
            cx.debug_bounds(selector).expect("card laid out")
        })
        .collect();
    (cards, cx.debug_bounds("slot").expect("slot laid out"))
}

fn ru(n: usize) -> String {
    "Сообщение не отправлено: чат переподключается к серверу Bridge. "
        .chars()
        .cycle()
        .take(n)
        .collect()
}

#[gpui::test]
fn a_long_message_wraps_inside_the_stack_width(cx: &mut TestAppContext) {
    let (cards, slot) = layout(
        cx,
        vec![toast("short", "Saved", &[]), toast("long", &ru(200), &[])],
    );
    let (short, long) = (cards[0], cards[1]);
    assert!(
        long.size.width <= px(SLOT),
        "long toast {:?} wider than the stack",
        long.size
    );
    assert!(
        long.size.height >= short.size.height + px(24.),
        "long toast did not wrap: {:?} vs one line {:?}",
        long.size,
        short.size
    );
    // Узкая карта не растянута и держится правого края.
    assert!(short.size.width < px(SLOT - 100.));
    assert_eq!(short.right(), slot.right());
    assert_eq!(long.right(), slot.right());
}

#[gpui::test]
fn a_message_without_spaces_breaks_inside_the_stack_width(cx: &mut TestAppContext) {
    let token = "x".repeat(400);
    let (cards, _) = layout(cx, vec![toast("token", &token, &["Retry", "Copy"])]);
    assert!(
        cards[0].size.width <= px(SLOT),
        "token toast {:?}",
        cards[0].size
    );
}

#[gpui::test]
fn a_very_long_message_stops_growing_at_its_line_limit(cx: &mut TestAppContext) {
    let (cards, _) = layout(
        cx,
        vec![toast("four", &ru(1000), &[]), toast("many", &ru(4000), &[])],
    );
    assert_eq!(
        cards[0].size, cards[1].size,
        "a clamped toast keeps the height of its line limit"
    );
}

#[gpui::test]
fn a_stack_of_eight_keeps_every_card_in_the_slot(cx: &mut TestAppContext) {
    let items = (0..8)
        .map(|i| {
            toast(
                &format!("t{i}"),
                &ru(80 + 40 * i),
                if i % 2 == 0 { &["Open"] } else { &[] },
            )
        })
        .collect();
    let (cards, slot) = layout(cx, items);
    for (i, card) in cards.iter().enumerate() {
        assert!(card.size.width <= px(SLOT), "card {i}: {:?}", card.size);
        assert_eq!(card.right(), slot.right(), "card {i} is not right-aligned");
    }
}
