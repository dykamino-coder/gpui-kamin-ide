//! Пункты меню: иконка, строка, разделитель, глифы.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

pub(crate) const MENU_W: f32 = 180.0;
pub(crate) const SUB_W: f32 = 180.0;
pub(crate) const MARGIN: f32 = 8.0;
// FontAwesome-глифы пунктов (fa-solid, как в оригинале file-context-menu.ts)
pub(crate) const FA_OPEN_IN: &str = "\u{f08e}"; // arrow-up-right-from-square
pub(crate) const FA_NEW_FILE: &str = "\u{e494}"; // file-circle-plus
pub(crate) const FA_NEW_FOLDER: &str = "\u{f65e}"; // folder-plus
pub(crate) const FA_CUT: &str = "\u{f0c4}"; // scissors
pub(crate) const FA_COPY: &str = "\u{f0c5}"; // copy
pub(crate) const FA_PASTE: &str = "\u{f0ea}"; // paste
pub(crate) const FA_PEN: &str = "\u{f304}"; // pen
pub(crate) const FA_TRASH: &str = "\u{f2ed}"; // trash-can
pub(crate) const FA_LINK: &str = "\u{f0c1}"; // link
pub(crate) const FA_ROUTE: &str = "\u{f4d7}"; // route
pub(crate) const FA_FOLDER_OPEN: &str = "\u{f07c}"; // folder-open
pub(crate) const FA_TERMINAL: &str = "\u{f120}"; // terminal
pub(crate) const FA_WINDOW: &str = "\u{f2d0}"; // window-maximize
/// `.danger { color: var(--accent-danger, #e5484d) }` — токен `--accent-danger`
/// в темах НЕ объявлен, поэтому у оригинала работает именно фолбэк-хекс
/// (это не accent-red #f38ba8).
fn danger_color() -> gpui::Rgba {
    gpui::rgb(0xe5484d)
}
/// Слот иконки .itemIcon: 16px фикс, глиф 12px по центру, muted
/// (danger-ряд красит иконку в цвет лейбла — inherit).
pub(crate) fn icon_slot(glyph: &'static str, danger: bool, p: &Palette) -> AnyElement {
    let color = if danger {
        danger_color()
    } else {
        rgba(p.text_muted)
    };
    div()
        .w(px(16.0))
        .flex_shrink_0()
        .flex()
        .justify_center()
        .when(!glyph.is_empty(), |d| {
            d.child(crate::ui::icon::fa(glyph, 12.0).text_color(color))
        })
        .into_any_element()
}
pub(crate) fn item(
    id: &'static str,
    glyph: &'static str,
    label: &'static str,
    danger: bool,
    p: &Palette,
    on_click: impl Fn() + 'static,
) -> AnyElement {
    item_owned(id, glyph, label.to_string(), danger, p, on_click)
}
/// .item (FileContextMenu.module.css): gap space-2, padding space-2/space-3,
/// radius-sm, fs-sm, text-primary; hover text-primary 10%;
/// .danger — accent-red текст+иконка, hover red 16%.
pub(crate) fn item_owned(
    id: &'static str,
    glyph: &'static str,
    label: String,
    danger: bool,
    p: &Palette,
    on_click: impl Fn() + 'static,
) -> AnyElement {
    let hover_bg = if danger {
        tint(danger_color(), 0.16)
    } else {
        tint(rgba(p.text_primary), 0.10)
    };
    let row = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_2))
        .rounded(px(m::RADIUS_SM))
        .text_size(px(m::FS_SM))
        .text_color(if danger {
            danger_color()
        } else {
            rgba(p.text_primary)
        })
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        // `onClick` (`FileContextMenu.tsx:107`), а не mouse-down: нажал и увёл
        // курсор — действие НЕ выполняется. Для строки Delete это критично
        // (ревью ц.25)
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            on_click();
        })
        .child(icon_slot(glyph, danger, p))
        .child(div().flex_1().whitespace_nowrap().child(label));
    // Пункты меню оригинала — `<button role="menuitem">`, значит таб-стопы с
    // `button:focus-visible` (`theme/global.css:38-43`). Кольцо ставим в ОБЩЕМ
    // конструкторе: так его получают ВСЕ пункты сразу (ревью ц.26)
    crate::ui::focus_ring::focusable(row, id, m::RADIUS_SM, rgba(p.accent_primary))
        .into_any_element()
}
pub(crate) fn divider(p: &Palette) -> AnyElement {
    div()
        .h(px(1.0))
        .mx(px(m::SPACE_2))
        .my(px(m::SPACE_1))
        .bg(tint(rgba(p.text_primary), 0.06))
        .into_any_element()
}
