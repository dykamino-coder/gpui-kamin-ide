//! Подменю «Move to ▸» у меню таба тула: список прочих слотов.
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`); оформление коробки
//! приходит замыканием `box_style` из `tool_menu` — оно там же общее с корневым
//! меню.

use crate::colors::rgba;
use crate::host_link::ShellEvent;
use gpui::prelude::*;
use gpui::px;
use kamin_metrics as m;

/// Строка сабменю: слот-назначение, его глиф и подпись.
pub(crate) type Entry = (
    crate::activity::PanelSlot,
    crate::ui::panel_placeholder::SlotIcon,
    &'static str,
);

#[allow(clippy::too_many_arguments)]
pub(crate) fn submenu(
    layer: gpui::Div,
    slot: crate::activity::PanelSlot,
    id: &str,
    entries: &[Entry],
    menu_w: f32,
    row_h: f32,
    sub_menu_h: impl Fn(usize) -> f32,
    box_style: impl Fn(gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div>,
    hover_bg: gpui::Rgba,
    x: f32,
    y: f32,
    vw: f32,
    vh: f32,
    tx: &smol::channel::Sender<ShellEvent>,
    p: &'static kamin_theme::Palette,
) -> gpui::Div {
    // Сабменю: все слоты, кроме текущего; высота считается так же, а не
    // по константе 240 (ревью ц.12)
    let sub_h = sub_menu_h(entries.len() - 1);
    // Якорь — СТРОКА «Move to», а не коробка меню: `a.left = menu.left + 1
    // (border) + 4 (padding)`, offset 4. Отсюда правая сторона
    // `a.right + 4 = x + menu_w − 1`, а флип — `a.left − 4 − w = x + 1 − w`
    // (у нас было `x − 4 − w`, то есть на 5 px левее; ревью ц.23).
    // `side: "right"` с флипом: справа не влезает, а слева влезает →
    // `left = a.left − offset − w` (`clamp-popup.ts:92-94`); кламп по X
    // накрывал родительское меню (ревью ц.21)
    let sub_right = x + menu_w - 1.0;
    let sub_left = x + 1.0 - menu_w;
    let sub_x = if sub_right + menu_w > vw - 8.0 && sub_left >= 8.0 {
        sub_left
    } else {
        sub_right
    };
    // `side: "right"` центрирует сабменю по СТРОКЕ-якорю:
    // `top = row.top + row.height/2 − sub.height/2` (`clamp-popup.ts:105`);
    // мы ставили его верхом на верх строки — на пяти пунктах это ~71px
    // выше (ревью ц.15). Верх строки = рамка 1 + паддинг 4 + первый пункт + gap 1.
    let row_top = y + 1.0 + m::SPACE_1 + row_h + 1.0;
    let sub_y = (row_top + row_h / 2.0 - sub_h / 2.0)
        .min(vh - sub_h - 8.0)
        .max(8.0);
    let mut sub = box_style(gpui::div().id("tool-tab-submenu"))
        .left(px(sub_x))
        .top(px(sub_y));
    for (dst, icon, label) in entries {
        if *dst == slot {
            continue;
        }
        let tx = tx.clone();
        let id = id.to_string();
        let dst = *dst;
        sub = sub.child(
            gpui::div()
                .id(gpui::SharedString::from(format!("ttm-{}", dst.as_str())))
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .rounded(px(m::RADIUS_SM))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_primary))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = tx.try_send(ShellEvent::MoveToolTo(slot, id.clone(), dst));
                    let _ = tx.try_send(ShellEvent::CloseToolTabMenu);
                })
                .child(crate::ui::panel_placeholder::slot_glyph_small(*icon, p))
                .child(gpui::div().flex_1().child(*label)),
        );
    }
    layer.child(sub)
}
