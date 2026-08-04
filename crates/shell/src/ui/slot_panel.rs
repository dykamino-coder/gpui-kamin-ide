//! Панель-слот (MainBottomPanel/RightPanel-карты 1:1): горизонтальный
//! BottomTabBar (табы pinned-тулзов + «...» пикер) + тело активного тула,
//! либо плейсхолдер с «Open Tool ▾» (открывает тот же пикер).
//! Тело активного тула строит вызывающий (root.rs) — сюда приходит готовым.

use crate::ui::slot_tabs::FA_CHEVRON_DOWN;
use crate::ui::slot_tabs::{dots, drop_placeholder, tab};
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::activity::{PanelSlot, PanelState};
use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::panel_placeholder::SlotIcon;

/// Пилюля «Open Tool ▾» пустого слота (accent-tint 16%).
pub(crate) fn open_tool_btn(
    slot: PanelSlot,
    up: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let base = tint(rgba(p.accent_primary), 0.16);
    let hover = tint(rgba(p.accent_primary), 0.26);
    let tx = tx.clone();
    div()
        .id(SharedString::from(format!("opentool-{}", slot.as_str())))
        // Тот же якорь: в пустом слоте пикер открывает именно пилюля
        .relative()
        .child(crate::probe::registry::probe_area(
            crate::ui::tool_picker::picker_anchor_id(slot),
        ))
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_1))
        .mt(px(m::SPACE_1))
        .rounded(px(m::RADIUS_SM))
        .bg(base)
        .text_size(px(m::FS_SM))
        .text_color(rgba(p.text_primary))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .on_mouse_down(
            gpui::MouseButton::Left,
            move |e: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::OpenToolPicker(
                    slot,
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                    up,
                ));
            },
        )
        .child("Open Tool")
        .child(crate::ui::icon::fa(FA_CHEVRON_DOWN, 10.0))
        .into_any_element()
}

/// Панель-слот: tabbar (pinned>0) + body|placeholder.
#[allow(clippy::too_many_arguments)]
pub fn slot_panel(
    slot: PanelSlot,
    state: &PanelState,
    label: &'static str,
    icon: SlotIcon,
    picker_up: bool,
    drag_over: Option<usize>,
    // id перетаскиваемого тула: `.tabDragging { opacity: .3 }`
    dragging: Option<&str>,
    body: Option<AnyElement>,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let mut col = div().flex().flex_col().size_full().min_h(px(0.));

    // Оригинал `BottomTabBar.tsx` рисует `.strip` ВСЕГДА (даже без табов —
    // остаётся пикер «…»), поэтому условия на непустой `pinned` нет
    {
        // `.strip` — gap space-1, padding 4px space-2 (сверху И снизу)
        let mut bar = div()
            .flex()
            .items_center()
            .gap(px(m::SPACE_1))
            .flex_shrink_0()
            .px(px(m::SPACE_2))
            .py(px(4.0))
            // `.strip { border-radius: var(--radius-sm) }`
            .rounded(px(m::RADIUS_SM));
        // `.tabs { flex: 1; min-width: 0; overflow-x: auto; gap: space-1 }` —
        // без контейнера переполнение резалось (ревью ц.9)
        let mut tabs = div()
            .id(SharedString::from(format!("tabs-{}", slot.as_str())))
            .flex()
            .items_center()
            .gap(px(m::SPACE_1))
            .flex_1()
            .min_w(px(0.))
            .overflow_x_scroll();
        for (i, id) in state.pinned.iter().enumerate() {
            if drag_over == Some(i) {
                tabs = tabs.child(drop_placeholder(p));
            }
            tabs = tabs.child(tab(
                slot,
                id,
                i,
                state.active.as_deref() == Some(id.as_str()),
                dragging == Some(id.as_str()),
                tx,
                p,
            ));
        }
        // Вставка в КОНЕЦ (overIndex == pinned.len())
        if drag_over == Some(state.pinned.len()) {
            tabs = tabs.child(drop_placeholder(p));
        }
        bar = bar.child(tabs).child(dots(slot, picker_up, tx, p));
        col = col.child(bar);
    }

    let content: AnyElement = match body {
        Some(el) => el,
        None => crate::ui::panel_placeholder::panel_placeholder_ex(
            label,
            "Open new tool or drag-n-drop tool from other panels",
            icon,
            // Оригинал: пилюля появляется только там, где placeholder получил
            // `activitySlot` — у центральной карты (`MainContent.tsx:55`) его нет
            if slot == PanelSlot::Main {
                None
            } else {
                Some(open_tool_btn(slot, picker_up, tx, p))
            },
            p,
        ),
    };
    col.child(div().flex_1().min_h(px(0.)).child(content))
        .into_any_element()
}
