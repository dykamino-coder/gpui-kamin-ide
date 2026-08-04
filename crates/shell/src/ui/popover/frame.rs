//! Каркас поповера: якорь под триггером, рамка, строки-переключатели.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;
use crate::ui::layout_popover::CHECK;
use crate::ui::layout_popover::POP_W;
use crate::ui::panel_placeholder::{SlotIcon, slot_glyph_small};
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Анкор поповера — `clampToViewport(side: "bottom")` (`clamp-popup.ts:74-112`):
/// flip вниз→вверх, ЕСЛИ снизу не помещается, а сверху помещается; затем
/// центровка по кросс-оси и кламп ТОЛЬКО по горизонтали (вертикального
/// клампа у стороны `bottom` в оригинале нет — он выдавливал поповер вверх).
///
/// Размер поповера берётся ИЗМЕРЕННЫЙ — с прошлого кадра через
/// `probe_registry` (аналог второго прохода `getBoundingClientRect` у
/// оригинала); до первого замера — фолбэк `POP_W` × 0 (ревью ц.15).
pub fn anchor_below(trigger: &'static str, popover: &'static str, vw: f32, vh: f32) -> (f32, f32) {
    const GUTTER: f32 = 8.0;
    const OFFSET: f32 = 6.0;
    let [ax, ay, aw, ah] =
        crate::probe::registry::bounds_of(trigger).unwrap_or([vw, 0.0, 0.0, 42.0]);
    let [_, _, pop_w, pop_h] =
        crate::probe::registry::bounds_of(popover).unwrap_or([0.0, 0.0, POP_W, 0.0]);
    let below = ay + ah + OFFSET;
    let above = ay - OFFSET - pop_h;
    let top = if below + pop_h > vh - GUTTER && above >= GUTTER {
        above
    } else {
        below
    };
    let left = (ax + aw / 2.0 - pop_w / 2.0).clamp(GUTTER, (vw - pop_w - GUTTER).max(GUTTER));
    (left, top)
}
pub fn popover_frame(
    id: &'static str,
    trigger: &'static str,
    vw: f32,
    vh: f32,
    p: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let (left, top) = anchor_below(trigger, id, vw, vh);
    div()
        .id(id)
        .occlude()
        .absolute()
        .top(px(top))
        // left от MAIN-вьюпорта (right-анкор к overlay-вьюпорту ехал бы)
        .left(px(left))
        .min_w(px(POP_W))
        // .menu max-height: calc(100vh - 16px)
        .max_h(px(vh - 16.0))
        // `.menu { overflow-y: auto }` — длинный список пресетов
        // скроллится, а не обрезается (ревью ц.13)
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(1.0))
        .p(px(m::SPACE_1))
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.06))
        .child(crate::overlay::hit_area())
        // Замер собственной коробки для анкора следующего кадра
        .child(crate::probe::registry::probe_area(id))
        .shadow(crate::overlay::dropdown_shadow())
}
#[allow(clippy::too_many_arguments)]
pub fn toggle_row(
    id: &'static str,
    slot: SlotIcon,
    label: &'static str,
    on: bool,
    disabled: bool,
    hint: Option<&'static str>,
    ev: ShellEvent,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // .menuItem: gap space-2, padding space-2/space-3, text-primary, fs-sm,
    // hover text-primary 10%; слева .check 16x16 r3 (on: accent + галка).
    let hover_bg = tint(rgba(p.text_primary), 0.10);
    let tx = tx.clone();
    let effective_on = on && !disabled;
    let checkbox = {
        let mut cb = div()
            .w(px(16.0))
            .h(px(16.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.0))
            .border_1();
        if effective_on {
            cb = cb
                .bg(rgba(p.accent_primary))
                .border_color(rgba(p.accent_primary))
                .child(codicon(CHECK, 12.0).text_color(rgba(p.accent_action_fg)));
        } else {
            cb = cb.border_color(rgba(p.bg_overlay));
        }
        cb
    };
    let mut row = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_2))
        .rounded(px(m::RADIUS_SM))
        .text_size(px(m::FS_SM))
        .text_color(rgba(if disabled {
            p.text_muted
        } else {
            p.text_primary
        }))
        .child(checkbox)
        .child(
            div()
                .flex_shrink_0()
                .text_color(rgba(p.text_muted))
                .when(disabled, |d| d.opacity(0.4))
                .child(slot_glyph_small(slot, p)),
        )
        .child(div().flex_1().child(label));
    if disabled {
        // `.menuItem[disabled] { cursor: not-allowed }` — курсор задаётся
        // именно у отключённой строки (ревью ц.23)
        row = row.cursor(gpui::CursorStyle::OperationNotAllowed);
        if let Some(h) = hint {
            // .itemHint: text-disabled (без opacity — ревью ц.1)
            row = row.child(
                div()
                    .text_size(px(m::FS_XS))
                    .text_color(rgba(p.text_disabled))
                    .child(h),
            );
        }
    } else {
        row = row
            .cursor_pointer()
            .hover(move |st| st.bg(hover_bg))
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation(); // поповер не закрывается на toggle
                let _ = tx.try_send(ev.clone());
            });
    }
    row.into_any_element()
}
/// .menuLabel: заголовок секции (uppercase, fs-xs, muted).
pub fn menu_label(text: &'static str, p: &Palette) -> AnyElement {
    div()
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_1))
        .text_size(px(m::FS_XS))
        // `letter-spacing: 0.04em` (`LayoutToggles.module.css:60`)
        .letter_spacing(px(m::FS_XS * 0.04))
        .text_color(rgba(p.text_muted))
        .child(text.to_uppercase())
        .into_any_element()
}
/// .menuItem секции Layouts: codicon + label, метрика как у toggle_row.
pub fn menu_item(
    id: &'static str,
    glyph: &'static str,
    label: &'static str,
    ev: ShellEvent,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let tx = tx.clone();
    let hover_bg = tint(rgba(p.text_primary), 0.10);
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_2))
        .rounded(px(m::RADIUS_SM))
        .text_size(px(m::FS_SM))
        .text_color(rgba(p.text_primary))
        .cursor_pointer()
        .hover(move |st| st.bg(hover_bg))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            let _ = tx.try_send(ev.clone());
        })
        // `.itemIcon` своего font-size не задаёт → каскад отдаёт базовые
        // `.codicon { font-size: 16px }` (skeleton.css:2), а не 12
        .child(codicon(glyph, 16.0).text_color(rgba(p.text_muted)))
        .child(label)
        .into_any_element()
}
