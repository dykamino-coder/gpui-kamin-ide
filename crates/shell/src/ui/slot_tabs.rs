//! Табы слота: строка таба, плейсхолдер дропа, меню «…».
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::activity::{PanelSlot, lookup_any};
use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

const MORE: &str = "\u{ea7c}"; // codicon-more (…)
pub(crate) const FA_CHEVRON_DOWN: &str = "\u{f078}";
/// Таб стрипа: label, active-подсветка, клик = активация,
/// drag_over = цель вставки reorder (accent-полоса слева).
/// Ключ probe-региона таба стрипа: `strip-<slot>-<index>`. Набор мал и
/// стабилен, интернируется один раз.
pub(crate) fn strip_tab_key(slot: PanelSlot, index: usize) -> &'static str {
    crate::activity::intern(&format!("strip-{}-{index}", slot.as_str()))
}
pub(crate) fn tab(
    slot: PanelSlot,
    id: &str,
    index: usize,
    active: bool,
    // `.tabDragging { opacity: .3 }` (`BottomTabBar.module.css:69`)
    is_dragging: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let tab_group = SharedString::from(format!("tab-group-{}-{index}", slot.as_str()));
    // `if (!item) return` — неизвестный id таб НЕ рисует; фантомный «Tool»
    // с шестерёнкой был отсебятиной (ревью ц.21)
    let Some((label, icon)) = lookup_any(id) else {
        return gpui::div().into_any_element();
    };
    // `.tabDragging { opacity: .3 }` — исходный таб гаснет на время драга
    let drag_dim = is_dragging;
    let hover_bg = tint(rgba(p.bg_surface), 0.5);
    let tx_press = tx.clone();
    let tx_over = tx.clone();
    let id_owned = id.to_string();
    let mut t = div()
        .id(SharedString::from(format!("st-{}-{id}", slot.as_str())))
        .flex()
        .items_center()
        .gap(px(6.0))
        .h(px(24.0))
        .px(px(10.0))
        .rounded(px(m::RADIUS_SM))
        .text_size(px(11.0))
        .letter_spacing(px(11.0 * 0.02))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(rgba(p.text_secondary))
        .cursor_pointer()
        .group(tab_group.clone())
        // `data-tooltip={item.label}` (`BottomTabBar.tsx:56`) — тултипа
        // у таба не было вовсе (ревью ц.21)
        .tooltip(crate::ui::tooltip::tooltip(label.clone()))
        // `.tabActive:hover` держит accent 16%
        .when(!active, |t| {
            t.hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
        })
        .on_mouse_down(
            gpui::MouseButton::Left,
            move |e: &gpui::MouseDownEvent, _, _| {
                // Press → возможный dnd; отпускание без движения = активация
                let _ = tx_press.try_send(ShellEvent::ToolPress(
                    slot,
                    id_owned.clone(),
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                ));
            },
        )
        // Регион таба — опора позиционного индекса (ширина у табов разная,
        // равномерным шагом её не вычислить)
        .child(crate::probe::registry::probe_area(strip_tab_key(
            slot, index,
        )))
        // Зажатая ЛКМ над табом → индекс вставки: ЛЕВАЯ половина = «перед»,
        // правая = «после» (`activity-dnd.ts:41-46` сравнивает clientX с
        // серединой бокса). Раньше слался голый `index`, и вставка в правую
        // половину была недостижима (ревью ц.14).
        .on_mouse_move(move |e: &gpui::MouseMoveEvent, _, _| {
            if e.pressed_button != Some(gpui::MouseButton::Left) {
                return;
            }
            let after = crate::probe::registry::bounds_of(strip_tab_key(slot, index))
                .map(|[bx, _, bw, _]| f32::from(e.position.x) > bx + bw / 2.0)
                .unwrap_or(false);
            let idx = if after { index + 1 } else { index };
            let _ = tx_over.try_send(ShellEvent::ToolDragOverTab(slot, idx));
        })
        // RMB → меню Hide / Move to ▸
        .on_mouse_down(gpui::MouseButton::Right, {
            let tx = tx.clone();
            let id = id.to_string();
            move |e: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::OpenToolTabMenu(
                    slot,
                    id.clone(),
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                ));
            }
        })
        .child({
            // Иконка тула в табе (оригинал BottomTabBar): phosphor-ассет
            // или codicon по имени из registry
            let icon_color = crate::colors::rgba(if active {
                p.text_primary
            } else {
                p.text_secondary
            });
            // Единый резолв (`ToolIcon`); в стрипе кегль 13.
            // `.tab:hover { color: text-primary }` красит и глиф — цвет
            // прибит аргументом, поэтому через group_hover (ревью ц.17)
            crate::ui::activity_bar::tool_glyph_group_hover(
                &icon,
                13.0,
                13.0,
                icon_color,
                tab_group.clone(),
                crate::colors::rgba(p.text_primary),
            )
        })
        .child(
            // `.tabLabel { overflow: hidden; text-overflow: ellipsis }`
            div()
                .min_w(px(0.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(label),
        );
    if active {
        t = t
            .bg(tint(rgba(p.accent_primary), 0.16))
            .text_color(rgba(p.text_primary));
    }
    if drag_dim {
        t = t.opacity(0.3);
    }
    t.into_any_element()
}
/// `.dropPlaceholder` стрипа: 36×24, r-sm, 1px dashed accent 70%, фон 14%.
pub(crate) fn drop_placeholder(p: &Palette) -> AnyElement {
    div()
        .flex_shrink_0()
        .w(px(36.0))
        .h(px(24.0))
        .rounded(px(m::RADIUS_SM))
        .border_1()
        .border_dashed()
        .border_color(tint(rgba(p.accent_primary), 0.7))
        .bg(tint(rgba(p.accent_primary), 0.14))
        .into_any_element()
}
/// «...» пикер-кнопка стрипа («Add or remove items»).
pub(crate) fn dots(slot: PanelSlot, up: bool, tx: &Sender<ShellEvent>, p: &Palette) -> AnyElement {
    let hover_bg = tint(rgba(p.bg_surface), 0.5);
    let tx = tx.clone();
    div()
        .id(SharedString::from(format!("dots-{}", slot.as_str())))
        // Якорь пикера — рект самой кнопки (ревью ц.15)
        .relative()
        .child(crate::probe::registry::probe_area(
            crate::ui::tool_picker::picker_anchor_id(slot),
        ))
        // `.picker` = 32×32 (высоту стрипа задаёт именно он: 4+32+4 = 40)
        .w(px(32.0))
        .h(px(32.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(m::RADIUS_SM))
        .text_color(rgba(p.text_muted))
        .cursor_pointer()
        .group("strip-dots-group")
        .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
        .tooltip(crate::ui::tooltip::tooltip("Add or remove items"))
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
        .child(
            crate::ui::icon::codicon(MORE, 18.0)
                .text_color(rgba(p.text_muted))
                .group_hover("strip-dots-group", {
                    let tp = rgba(p.text_primary);
                    move |st| st.text_color(tp)
                }),
        )
        .into_any_element()
}
