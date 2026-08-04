//! Шапка панели файлов: заголовок и кнопки инструментов.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::probe::registry::probe_area;
use crate::ui::file_tree::model::TreeState;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Кнопка тулбара дерева (FileTreeHeader .btn: 22×22, grid center, glyph 14).
pub(crate) fn tool_btn(
    id: &'static str,
    glyph: &'static str,
    tip: impl Into<SharedString>,
    disabled: bool,
    p: &Palette,
    on_click: impl Fn() + 'static,
) -> AnyElement {
    let btn_group = SharedString::from(format!("ftbtn-{glyph}"));
    let hover_bg = {
        let mut c = rgba(p.bg_surface);
        c.a = 0.6;
        c
    };
    let mut b = div()
        .id(id)
        .w(px(22.0))
        .h(px(22.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(m::RADIUS_XS))
        .text_color(rgba(p.text_muted))
        .group(btn_group.clone())
        .child(
            // `.btn:hover:not([disabled]) { color: text-primary }` красит и
            // ГЛИФ; собственный `.hover()` до него не доходит (замеры ц.21)
            codicon(glyph, 14.0)
                .text_color(rgba(p.text_muted))
                .group_hover(btn_group, {
                    let tp = rgba(p.text_primary);
                    move |st| st.text_color(tp)
                }),
        );
    let tip: SharedString = tip.into();
    if disabled {
        // `[disabled] { opacity: .4; cursor: not-allowed }` — тултип остаётся
        b = b
            .opacity(0.4)
            .cursor(gpui::CursorStyle::OperationNotAllowed)
            .tooltip(crate::ui::tooltip::tooltip(tip));
    } else {
        b = b
            .cursor_pointer()
            .tooltip(crate::ui::tooltip::tooltip(tip))
            .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| on_click());
    }
    // `:focus-visible`: кольцо только у активной кнопки — `[disabled]` в
    // браузере из таб-порядка выпадает
    if disabled {
        return b.into_any_element();
    }
    crate::ui::focus_ring::focusable(
        b,
        &format!("treebtn:{id}"),
        m::RADIUS_XS,
        rgba(p.accent_primary),
    )
    .into_any_element()
}
/// FileTreeHeader 1:1: титул (uppercase, тултип = полный путь; «PROJECT» без
/// воркспейса), бейдж Indexing и три кнопки. Кнопки блокируются ровно как в
/// оригинале: Locate — без воркспейса или без выделения, Collapse/Refresh —
/// без воркспейса. Collapse — ТУМБЛЕР: глиф и подпись меняются по
/// `treeAllCollapsed` (`FileTreeHeader.tsx:22-76`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn tree_header(
    workspace: Option<&str>,
    tree: &TreeState,
    all_collapsed: bool,
    // Есть ли активный файл редактора: от него зависит Locate
    // (`!selectedFile.value` оригинала, а не выделение дерева)
    has_editor_file: bool,
    p: &Palette,
    on_locate: impl Fn() + 'static,
    on_collapse_all: impl Fn() + 'static,
    on_refresh: impl Fn() + 'static,
) -> AnyElement {
    let title = workspace
        .map(|root| {
            root.replace('\\', "/")
                .rsplit('/')
                .next()
                .unwrap_or(root)
                .to_string()
        })
        .unwrap_or_else(|| "PROJECT".to_string());
    let no_root = workspace.is_none();
    let mut title_el = div()
        .id("file-tree-title")
        .flex_1()
        .min_w(px(0.))
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .text_size(px(m::FS_XS))
        // `letter-spacing: 0.08em` (`FileTreeHeader.module.css:13`)
        .letter_spacing(px(m::FS_XS * 0.08))
        .font(crate::ui::typo::ss01(gpui::FontWeight::MEDIUM))
        .text_color(rgba(p.text_muted))
        // `text-overflow: ellipsis` (`FileTreeHeader.module.css:16-17`):
        // движок «…» не рисует, дописываем сами — как у строк дерева.
        // Бюджет = ширина панели минус паддинги 8+12 и три кнопки 22 с гэпом 4
        // (ревью ц.25: титул просто обрезался)
        .child(crate::ui::text_fit::fit_approx(
            &title.to_uppercase(),
            (crate::probe::registry::bounds_of("file-tree")
                .map(|[_, _, w, _]| w)
                .unwrap_or(240.0)
                - 20.0
                - 3.0 * 22.0
                - 2.0 * 4.0
                - m::SPACE_1)
                .max(24.0),
            m::FS_XS,
        ));
    if let Some(root) = workspace {
        // `data-tooltip={root}` — полный путь воркспейса
        title_el = title_el.tooltip(crate::ui::tooltip::tooltip(root.to_string()));
    }
    div()
        .relative()
        .child(probe_area("file-tree-header"))
        .flex()
        .items_center()
        .gap(px(m::SPACE_1))
        .pl(px(12.0))
        .pr(px(m::SPACE_2))
        .py(px(m::SPACE_2))
        .flex_shrink_0()
        .child(title_el)
        .when(tree.indexing, |h| {
            h.child(
                div()
                    .id("file-tree-indexing")
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .flex_shrink_0()
                    .text_size(px(m::FS_XS))
                    .text_color(rgba(p.text_muted))
                    .opacity(0.85)
                    .tooltip(crate::ui::tooltip::tooltip(
                        "Building the search index (Ctrl+P)…",
                    ))
                    // `codicon-loading codicon-modifier-spin`: глиф крутится
                    .child(crate::ui::icon::spinner(
                        "tree-indexing-spin",
                        12.0,
                        rgba(p.text_muted),
                    ))
                    .child("Indexing…"),
            )
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(2.0))
                .flex_shrink_0()
                .child(tool_btn(
                    "tree-locate",
                    "\u{ebf8}",
                    "Locate selected file",
                    no_root || !has_editor_file,
                    p,
                    on_locate,
                ))
                .child(tool_btn(
                    "tree-collapse",
                    if all_collapsed {
                        "\u{eb95}" // codicon-expand-all
                    } else {
                        "\u{eac5}" // codicon-collapse-all
                    },
                    if all_collapsed {
                        "Expand all folders"
                    } else {
                        "Collapse all folders"
                    },
                    no_root,
                    p,
                    on_collapse_all,
                ))
                .child(tool_btn(
                    "tree-refresh",
                    "\u{eb37}",
                    "Refresh",
                    no_root,
                    p,
                    on_refresh,
                )),
        )
        .into_any_element()
}
