//! Плитка тула в полосе активности.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::host_link::ShellEvent;
use crate::ui::activity::glyphs::{activity_tooltip, tool_glyph_group_hover};
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

pub fn tile(
    id: &'static str,
    icon: &'static str,
    is_active: bool,
    // Плитка сейчас перетаскивается — гаснет до .3 (`ActivityBar.module.css:17`)
    is_dragging: bool,
    p: &Palette,
    on_click: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
    // (слот, id тула, индекс, канал) — плитка как источник и цель drag'а
    drag: Option<(
        crate::activity::PanelSlot,
        String,
        usize,
        Sender<ShellEvent>,
    )>,
) -> AnyElement {
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
    let group = SharedString::from(format!("tile-group-{id}"));
    let icon_el = tool_glyph_group_hover(
        icon,
        18.0,
        18.0,
        icon_color,
        group.clone(),
        rgba(p.text_primary),
    );
    let mut btn = div()
        .id(id)
        .group(group)
        .w(px(32.0))
        .h(px(32.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(m::RADIUS_SM))
        .cursor_pointer()
        .text_color(rgba(if is_active {
            p.text_primary
        } else {
            p.text_muted
        }))
        // `.btnActive:hover` держит accent 16% → ховер только у неактивной
        .when(!is_active, |b| {
            b.hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
        })
        .when(!activity_tooltip(id).is_empty(), |b| {
            b.tooltip(crate::ui::tooltip::tooltip(activity_tooltip(id)))
        })
        // Активация по нажатию — ТОЛЬКО у плиток без drag-модели (gear,
        // «…»). У перетаскиваемых активация происходит на отпускании без
        // движения (`activity-dnd.ts:88-100`), иначе старт драга сразу
        // переключал тело сайдбара (ревью ц.12)
        .when(drag.is_none(), |b| {
            b.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                on_click(window, cx)
            })
        })
        .child(icon_el);
    // `.tileDragging > .btn { opacity: .3 }` — исходная плитка гаснет, пока
    // её ghost едет за курсором (`ActivityBar.module.css:17-19`)
    if is_dragging {
        btn = btn.opacity(0.3);
    }
    // Поэлементный кроп parity/shots.py: регион у ПЕРВОЙ плитки списка
    if matches!(&drag, Some((_, _, 0, _))) {
        btn = btn
            .relative()
            .child(crate::probe::registry::probe_area("activity-tile"));
    }
    if let Some((slot, tool_id, index, tx)) = drag {
        let tx_press = tx.clone();
        let tx_menu = tx.clone();
        let id_press = tool_id.clone();
        btn = btn
            // Press → возможный dnd (отпускание без движения = активация,
            // её уже делает on_click выше)
            .on_mouse_down(
                gpui::MouseButton::Left,
                move |e: &gpui::MouseDownEvent, _, _| {
                    let _ = tx_press.try_send(ShellEvent::ToolPress(
                        slot,
                        id_press.clone(),
                        f32::from(e.position.x),
                        f32::from(e.position.y),
                    ));
                },
            )
            // Зажатая ЛКМ над плиткой → индекс вставки: верхняя половина
            // плитки означает «перед ней», нижняя — «после» (`activity-dnd.ts`
            // сравнивает clientY с серединой бокса). Без этого вставка в конец
            // была недостижима.
            .on_mouse_move(move |e: &gpui::MouseMoveEvent, window, _| {
                if e.pressed_button != Some(gpui::MouseButton::Left) {
                    return;
                }
                let after = crate::probe::registry::bounds_of("activity-tile")
                    .map(|[_, ty, _, th]| {
                        // регион первой плитки задаёт шаг: y плитки index
                        let step = th + 2.0;
                        let mid = ty + step * index as f32 + th / 2.0;
                        f32::from(e.position.y) > mid
                    })
                    .unwrap_or(false);
                let _ = window;
                let idx = if after { index + 1 } else { index };
                let _ = tx.try_send(ShellEvent::ToolDragOverTab(slot, idx));
            })
            // RMB по плитке — меню тула (Hide / Move to ▸), как у правого рейла
            .on_mouse_down(gpui::MouseButton::Right, {
                let id_menu = tool_id.clone();
                move |e: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    let _ = tx_menu.try_send(ShellEvent::OpenToolTabMenu(
                        slot,
                        id_menu.clone(),
                        f32::from(e.position.x),
                        f32::from(e.position.y),
                    ));
                }
            });
    }
    if is_active {
        btn = btn.bg(active_bg);
    }
    // `:focus-visible`: кольцо accent-primary при переходе по Tab
    crate::ui::focus_ring::focusable(
        btn,
        &format!("activity:{id}"),
        m::RADIUS_SM,
        rgba(p.accent_primary),
    )
    .into_any_element()
}
