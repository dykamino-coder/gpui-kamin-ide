//! Шапка панели Problems: счётчики ошибок и предупреждений, фильтры.
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::host_link::ShellEvent;
use crate::ui::problems_parts::count_btn;
use gpui::prelude::*;
use gpui::{div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

pub(crate) fn header(
    errors: usize,
    warnings: usize,
    filter: Option<u8>,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> gpui::Div {
    let header = div()
        .relative()
        .child(crate::probe::registry::probe_area("problems-header"))
        .flex()
        .items_center()
        .justify_between()
        .flex_shrink_0()
        .pl(px(m::SPACE_3))
        .pr(px(m::SPACE_2))
        .py(px(m::SPACE_2))
        .text_size(px(m::FS_XS))
        .letter_spacing(px(m::FS_XS * 0.08))
        .font(crate::ui::typo::ss01(gpui::FontWeight::MEDIUM))
        .text_color(rgba(p.text_muted))
        .child("PROBLEMS")
        .child(
            div()
                // `.counts { letter-spacing: 0 }` — счётчики трекинг хедера
                // не наследуют
                .letter_spacing(px(0.))
                .flex()
                .gap(px(4.0))
                .child(count_btn(
                    0,
                    "\u{ea87}",
                    errors,
                    filter == Some(0),
                    rgba(p.accent_red),
                    "Filter errors",
                    tx,
                    p,
                ))
                .child(count_btn(
                    1,
                    "\u{ea6c}",
                    warnings,
                    filter == Some(1),
                    rgba(p.accent_yellow),
                    "Filter warnings",
                    tx,
                    p,
                )),
        );

    // Файлы: фильтр severity → пустые скрыты; сортировка по uri; кап
    header
}
