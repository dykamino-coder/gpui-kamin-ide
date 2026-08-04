//! Пункты меню от расширений.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::EdEvent;
use crate::host::events::ShellEvent;
use crate::ui::fmenu::items::{divider, icon_slot};
use crate::ui::fmenu::model::ContribMenuItem;
use crate::ui::fmenu::model::{group_key, when_allows};
use gpui::prelude::*;
use gpui::{SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Пункты, добавленные расширениями (сепаратор на смене группы).
#[allow(clippy::too_many_arguments)]
pub(crate) fn contributed_items(
    mut rest: gpui::Stateful<gpui::Div>,
    p: &Palette,
    tx: &Sender<ShellEvent>,
    path: &str,
    is_dir: bool,
    contributed: &[ContribMenuItem],
) -> gpui::Stateful<gpui::Div> {
    // ── Contributed explorer/context (после builtin; сепаратор на смене
    // группы; клик = command:execute с marshalled Uri {$mid:1})
    let mut visible: Vec<&ContribMenuItem> = contributed
        .iter()
        .filter(|c| when_allows(&c.when, path, is_dir))
        .collect();
    visible.sort_by_key(|c| group_key(&c.group));
    let mut last_group: Option<String> = None;
    for (ci, c) in visible.into_iter().enumerate() {
        let (gk, _) = group_key(&c.group);
        if ci == 0 || last_group.as_deref() != Some(gk.as_str()) {
            rest = rest.child(divider(p));
            last_group = Some(gk);
        }
        let hover_bg = tint(rgba(p.text_primary), 0.10);
        let tx = tx.clone();
        let command = c.command.clone();
        let path = path.to_string();
        rest = rest.child(
            div()
                .id(SharedString::from(format!("fm-contrib-{ci}")))
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
                // `onClick`, как и у встроенных пунктов (ревью ц.25)
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    let command = command.clone();
                    let arg = serde_json::json!({
                        "$mid": 1, "scheme": "file",
                        "fsPath": path.replace('/', "\\"),
                    });
                    std::thread::spawn(move || {
                        if let Some(cl) = crate::host_link::client() {
                            let _ = cl.request(
                                "kamin:command:execute",
                                vec![
                                    serde_json::json!(command),
                                    arg.clone(),
                                    serde_json::json!([arg]),
                                ],
                            );
                        }
                    });
                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
                })
                .child(icon_slot("", false, p))
                .child(div().flex_1().whitespace_nowrap().child(c.label.clone())),
        );
    }
    rest
}
