//! Содержимое строки contributed-дерева: шеврон, чекбокс, иконка, метка.
//!
//! Вынесено из `level.rs` без изменения поведения
//! (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::host_link::ShellEvent;
use crate::ui::contributed_tree::{CHEVRON_DOWN, CHEVRON_RIGHT};
use crate::ui::ctree::nodes::{checkbox, node_icon};
use crate::ui::ctree::types::TreeNodeDto;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

#[allow(clippy::too_many_arguments)]
pub(crate) fn row_body(
    row: gpui::Stateful<gpui::Div>,
    node: &TreeNodeDto,
    view: &str,
    expandable: bool,
    expanded: bool,
    selected: bool,
    row_group: SharedString,
    panel_w: f32,
    depth: f32,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let mut row = row;
    // `.chevron` / `.chevronSpacer` — бокс 16, глиф 13, text-muted;
    // у выделенной строки цвет наследуется
    row = row.child(if expandable {
        let mut ch = div()
            .flex_shrink_0()
            .w(px(16.0))
            .flex()
            .justify_center()
            .child(codicon(
                if expanded {
                    CHEVRON_DOWN
                } else {
                    CHEVRON_RIGHT
                },
                // `.chevron{13px}` проигрывает базе codicon → 16 (ревью ц.13)
                16.0,
            ));
        if !selected {
            ch = ch.text_color(rgba(p.text_muted));
        }
        ch
    } else {
        div().flex_shrink_0().w(px(16.0)).h(px(16.0))
    });
    if let Some(state) = node.checkbox {
        // currentColor строки: у выделенной — text-primary
        let cb_color = if selected {
            rgba(p.text_primary)
        } else {
            rgba(p.text_secondary)
        };
        row = row.child(checkbox(
            view,
            node,
            state,
            cb_color,
            row_group.clone(),
            rgba(p.text_primary),
            rgba(p.accent_primary),
            tx,
        ));
    }
    row = row
        .child(node_icon(node, expanded))
        .child(
            div()
                // `.row:hover { color: text-primary }` красит лейбл;
                // ховер строки до вложенного текста не доходит (ц.21)
                .group_hover(row_group.clone(), {
                    let tp = rgba(p.text_primary);
                    move |st| st.text_color(tp)
                })
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                // Многоточие дописывает `text_fit` (движок его не
                // рисует); бюджет — ширина панели минус индент, шеврон,
                // иконка и паддинги (ревью ц.21)
                .child(crate::ui::text_fit::fit_approx(
                    &node.label,
                    panel_w - (depth * 12.0 + 8.0) - 52.0,
                    m::FS_SM,
                )),
        )
        .when_some(node.description.clone(), |d, desc| {
            // инлайн-стиль оригинала: opacity .55, ml 6, 0.85em
            d.child(
                div()
                    .ml(px(6.0))
                    .opacity(0.55)
                    .text_size(px(m::FS_SM * 0.85))
                    .child(SharedString::from(desc)),
            )
        });
    row
}
