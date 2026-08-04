//! Рейка правой колонки: плитки тулов.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::activity::{PanelSlot, PanelState, lookup_any};
use crate::colors::rgba;
use crate::host_link::ShellEvent;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Плитка рейла (32×32): press → ToolPress (dnd/активация в root),
/// RMB → меню Hide / Move to (та же механика, что у стрип-табов).
/// Ключ probe-региона первой плитки рейла (опора расчёта индекса вставки).
fn first_tile_key(slot: PanelSlot) -> &'static str {
    match slot {
        PanelSlot::RightTop => "rail-tile-right-top",
        PanelSlot::RightBottom => "rail-tile-right-bottom",
        _ => "rail-tile-other",
    }
}
fn rail_tile(
    slot: PanelSlot,
    id: &str,
    index: usize,
    is_active: bool,
    // Плитка сейчас перетаскивается: `.tileDragging > .btn { opacity: .3 }`
    is_dragging: bool,
    p: &Palette,
    tx: &Sender<ShellEvent>,
) -> AnyElement {
    // Неизвестный id оригинал ПРОПУСКАЕТ (`ActivityBar.tsx:79`), а не рисует
    // фантом `gear`/`Tool` (ревью ц.23)
    let Some((label, icon)) = lookup_any(id) else {
        return div().into_any_element();
    };
    let hover_bg = {
        let mut c = rgba(p.bg_surface);
        c.a = 0.5;
        c
    };
    let active_bg = {
        let mut c = rgba(p.accent_primary);
        c.a = 0.16;
        c
    };
    let icon_color = rgba(if is_active {
        p.text_primary
    } else {
        p.text_muted
    });
    // Единый резолв иконки тула (`ToolIcon` оригинала); плитка рейла — 18.
    // Ховер плитки красит и глиф — тот же `.btn:hover { color: text-primary }`,
    // что у бара (ревью ц.23)
    let rail_group = SharedString::from(format!("rail-g-{}-{id}", slot.as_str()));
    let icon_el: AnyElement = crate::ui::activity_bar::tool_glyph_group_hover(
        &icon,
        18.0,
        18.0,
        icon_color,
        rail_group.clone(),
        rgba(p.text_primary),
    );
    let press_tx = tx.clone();
    let press_id = id.to_string();
    let menu_tx = tx.clone();
    let menu_id = id.to_string();
    let mut t = div()
        .id(SharedString::from(format!("rail-{}-{id}", slot.as_str())))
        .group(rail_group)
        .w(px(32.0))
        .h(px(32.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(m::RADIUS_SM))
        .cursor_pointer()
        .tooltip(crate::ui::tooltip::tooltip(label.clone()))
        // `.btnActive:hover` держит accent → ховер только у неактивной
        .when(!is_active, |t| t.hover(move |s| s.bg(hover_bg)))
        .on_mouse_down(
            gpui::MouseButton::Left,
            move |e: &gpui::MouseDownEvent, _, _| {
                let _ = press_tx.try_send(ShellEvent::ToolPress(
                    slot,
                    press_id.clone(),
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                ));
            },
        )
        // Зажатая ЛКМ над плиткой рейла → индекс вставки: верхняя половина
        // «перед», нижняя «после» (`activity-dnd.ts:34-46`)
        .on_mouse_move({
            let tx_over = tx.clone();
            move |e: &gpui::MouseMoveEvent, _, _| {
                if e.pressed_button != Some(gpui::MouseButton::Left) {
                    return;
                }
                // Верх рейла даёт первая плитка (её регион), шаг = 32 + gap 2
                let after = crate::probe::registry::bounds_of(first_tile_key(slot))
                    .map(|[_, ty, _, th]| {
                        let mid = ty + (th + 2.0) * index as f32 + th / 2.0;
                        f32::from(e.position.y) > mid
                    })
                    .unwrap_or(false);
                let idx = if after { index + 1 } else { index };
                let _ = tx_over.try_send(ShellEvent::ToolDragOverTab(slot, idx));
            }
        })
        .on_mouse_down(
            gpui::MouseButton::Right,
            move |e: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                let _ = menu_tx.try_send(ShellEvent::OpenToolTabMenu(
                    slot,
                    menu_id.clone(),
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                ));
            },
        )
        .child(icon_el);
    // Регион ПЕРВОЙ плитки — опора для расчёта индекса вставки при драге
    if index == 0 {
        t = t
            .relative()
            .child(crate::probe::registry::probe_area(first_tile_key(slot)));
    }
    if is_dragging {
        t = t.opacity(0.3);
    }
    if is_active {
        t = t.bg(active_bg);
    }
    t.into_any_element()
}
/// Вертикальный rail карты: 48px, тайлы 32×32 из pinned-модели слота.
pub(crate) fn rail(
    slot: PanelSlot,
    state: &PanelState,
    // Индекс вставки при драге над этим слотом + id перетаскиваемого тула
    drop_index: Option<usize>,
    dragging: Option<&str>,
    p: &Palette,
    bottom: bool,
    tx: &Sender<ShellEvent>,
) -> AnyElement {
    // Оригинал (ActivityBar.module.css): `.bar` gap 8 — только между
    // группами (list ↔ picker), а `.list` внутри = gap 2px между плитками.
    let mut r = div()
        .flex_shrink_0()
        .w(px(m::ACTIVITY_BAR_WIDTH))
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(m::SPACE_2))
        .py(px(m::SPACE_3))
        // Рейл — такая же дроп-зона слота, как карта (`data-activity-slot`)
        .relative()
        .children(crate::ui::drop_hint::card_drop_r(
            drop_index.is_some(),
            false,
            m::RADIUS_MD,
            p,
        ))
        .child(crate::probe::registry::probe_area(match slot {
            PanelSlot::RightTop => "rail-right-top",
            PanelSlot::RightBottom => "rail-right-bottom",
            _ => "rail-other",
        }));
    if bottom {
        r = r.justify_end();
    }

    let mut list = div().flex().flex_col().items_center().w_full().gap(px(2.0));
    for (i, id) in state.pinned.iter().enumerate() {
        if drop_index == Some(i) {
            list = list.child(crate::ui::activity_bar::drop_placeholder_el(p));
        }
        list = list.child(rail_tile(
            slot,
            id,
            i,
            state.active.as_deref() == Some(id.as_str()),
            dragging == Some(id.as_str()),
            p,
            tx,
        ));
    }
    if drop_index == Some(state.pinned.len()) {
        list = list.child(crate::ui::activity_bar::drop_placeholder_el(p));
    }

    let picker = {
        let tx = tx.clone();
        let hover_bg = {
            let mut c = rgba(p.bg_surface);
            c.a = 0.5;
            c
        };
        div()
            .id(SharedString::from(format!("rail-dots-{}", slot.as_str())))
            .w(px(32.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(m::RADIUS_SM))
            .text_color(rgba(p.text_muted))
            .cursor_pointer()
            .tooltip(crate::ui::tooltip::tooltip("Add or remove items"))
            .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
            .on_mouse_down(gpui::MouseButton::Left, {
                let up = bottom;
                move |e: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    let _ = tx.try_send(ShellEvent::OpenToolPicker(
                        slot,
                        f32::from(e.position.x),
                        f32::from(e.position.y),
                        up,
                    ));
                }
            })
            .child(
                div()
                    .font_family("codicon")
                    .text_size(px(18.0))
                    .child("\u{ea7c}"),
            )
    };

    // Порядок как в оригинале: top = {list, picker}; bottom (justify_end) = {picker, list}
    if bottom {
        r = r.child(picker).child(list);
    } else {
        r = r.child(list).child(picker);
    }
    r.into_any_element()
}
