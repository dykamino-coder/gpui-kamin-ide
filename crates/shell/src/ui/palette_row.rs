//! Строка палитры команд.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::palette_filter::CommandItem;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

pub(crate) fn command_row(
    c: &CommandItem,
    first: bool,
    tx: &smol::channel::Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let hover = tint(rgba(p.accent_primary), 0.18);
    let base = if first {
        tint(rgba(p.accent_primary), 0.12)
    } else {
        gpui::transparent_black().into()
    };
    let tx = tx.clone();
    let id = c.id.clone();
    // `.title { flex: 1 }` — ни `nowrap`, ни `text-overflow` у оригинала нет,
    // длинный заголовок ПЕРЕНОСИТСЯ (ревью ц.35)
    let mut title_row = div().flex().min_w(px(0.)).flex_1();
    if let Some(cat) = &c.category {
        title_row = title_row.child(
            div()
                .flex_shrink_0()
                .text_color(rgba(p.text_muted))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(format!("{cat}: ")),
        );
    }
    title_row = title_row.child(
        div()
            .min_w(px(0.))
            .text_color(rgba(p.text_primary))
            .child(c.title.clone()),
    );
    let row_id = format!("cmd-{}", c.id);
    let row = div()
        .id(SharedString::from(row_id.clone()))
        .flex()
        .items_baseline()
        .justify_between()
        .gap(px(m::SPACE_3))
        .w_full()
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_2))
        .rounded(px(m::RADIUS_SM))
        .bg(base)
        .text_size(px(m::FS_MD))
        .cursor_pointer()
        // `.list > li:first-child .row` (0,3,1) перебивает `.row:hover`
        // (0,2,0): у первой строки ховер НЕ поднимает фон до 18 %
        // (ревью ц.17)
        .when(!first, |r| r.hover(move |s| s.bg(hover)))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            let _ = tx.try_send(ShellEvent::RunCommand(id.clone()));
        })
        .child(title_row)
        .child(
            // `.id` НЕ помечен `flex-shrink: 0` — при длинном заголовке
            // сжимается он, а не строка (ревью ц.35)
            div()
                .min_w(px(0.))
                .font_family("JetBrains Mono")
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                .child(c.id.clone()),
        );
    // Строки палитры оригинала — `<button>`, значит таб-стопы с
    // `button:focus-visible` (`theme/global.css:38-43`), ревью ц.26
    crate::ui::focus_ring::focusable(row, &row_id, m::RADIUS_SM, rgba(p.accent_primary))
        .into_any_element()
}
