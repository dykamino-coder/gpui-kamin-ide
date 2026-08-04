//! Problems-панель (ProblemsPanel/ProblemRow 1:1): хедер «PROBLEMS» +
//! счётчики-фильтры err/warn, группы по файлу (chevron-collapse, TreeIcon,
//! dirname, пилюля-счётчик), строки min-h22 pl26 (sev 14 + message + origin +
//! [Ln, Col]), капы 100 файлов / 200 строк + «Show more».

use crate::host::events::CzEvent;
pub use crate::ui::problems_diag::Diag;
use crate::ui::problems_parts::{FILE_CAP_STEP, ROW_CAP};
use std::collections::{HashMap, HashSet};

use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;

pub(crate) fn base_name(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_string()
}

/// Панель Problems: хедер + группы + капы. Стейт (фильтр/collapse/cap) — RootView.
pub fn problems_panel(
    diags: &HashMap<(String, String), Vec<Diag>>,
    filter: Option<u8>,
    collapsed: &HashSet<String>,
    file_cap: usize,
    tx: &Sender<ShellEvent>,
    p: &'static Palette,
) -> AnyElement {
    // Слить владельцев по uri
    let mut by_uri: HashMap<&str, Vec<&Diag>> = HashMap::new();
    for ((_, uri), list) in diags {
        by_uri.entry(uri).or_default().extend(list.iter());
    }
    let (mut errors, mut warnings) = (0usize, 0usize);
    for list in by_uri.values() {
        for d in list {
            match d.severity {
                0 => errors += 1,
                1 => warnings += 1,
                _ => {}
            }
        }
    }

    // .header: «PROBLEMS» 11/500/ls .08em + counts
    let header = crate::ui::problems_header::header(errors, warnings, filter, tx, p);
    let mut keys: Vec<&str> = by_uri.keys().copied().collect();
    keys.sort_unstable();
    let mut files: Vec<(&str, Vec<&Diag>)> = Vec::new();
    for uri in keys {
        let mut list: Vec<&Diag> = by_uri.remove(uri).unwrap_or_default();
        if let Some(sev) = filter {
            list.retain(|d| d.severity == sev);
        }
        if list.is_empty() {
            continue;
        }
        list.sort_by_key(|d| (d.severity, d.line));
        files.push((uri, list));
    }
    let total_files = files.len();
    let hidden_files = total_files.saturating_sub(file_cap);

    // .list: pb8, fs-sm
    let mut list_el = div()
        .id("problems-body")
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .overflow_y_scrollbar()
        .pb(px(m::SPACE_2))
        .text_size(px(m::FS_SM));

    if total_files == 0 {
        list_el = list_el.child(
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(px(m::SPACE_5))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_muted))
                .text_center()
                .child("No problems have been detected in the workspace."),
        );
    }

    for (uri, list) in files.into_iter().take(file_cap) {
        let is_collapsed = collapsed.contains(uri);
        let file_row = crate::ui::problems_rows::file_row(
            uri,
            &base_name(uri),
            list.len(),
            is_collapsed,
            tx,
            p,
        );
        list_el = list_el.child(file_row);
        if is_collapsed {
            continue;
        }
        let row_total = list.len();
        for (i, d) in list.into_iter().take(ROW_CAP).enumerate() {
            list_el = list_el.child(crate::ui::problems_rows::diag_row(uri, i, d, tx, p));
        }
        if row_total > ROW_CAP {
            list_el = list_el.child(
                div()
                    .pl(px(28.0))
                    .py(px(2.0))
                    .text_size(px(m::FS_XS))
                    .text_color(rgba(p.text_muted))
                    .child(format!(
                        "… {} more problems in this file",
                        row_total - ROW_CAP
                    )),
            );
        }
    }
    if hidden_files > 0 {
        let tx = tx.clone();
        list_el = list_el.child(
            div()
                .id("prob-show-more")
                .flex()
                .items_center()
                .gap(px(6.0))
                .w_full()
                .px(px(10.0))
                .py(px(6.0))
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                .cursor_pointer()
                .hover({
                    let hb = tint(rgba(p.bg_surface), 0.55);
                    move |s| s.bg(hb).text_color(rgba(p.text_primary))
                })
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = tx.try_send(ShellEvent::Cz(CzEvent::ProblemsShowMore));
                })
                .child(codicon("\u{ea7c}", 16.0)) // ellipsis: у `.showMore` кегля нет
                .child(format!(
                    "Show {} more files ({} hidden)",
                    hidden_files.min(FILE_CAP_STEP),
                    hidden_files
                )),
        );
    }

    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h(px(0.))
        .child(header)
        .child(list_el)
        .into_any_element()
}
