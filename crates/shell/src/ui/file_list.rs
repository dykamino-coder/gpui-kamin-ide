//! Дерево файлов (FileTreeView 1:1 по метрикам plan/23): ленивые листинги
//! per-dir (kamin:fs:listDir), раскрытие по клику, индент depth*12+8,
//! chevron для папок. Используется файловой панелью И right-top картой.

pub use crate::ui::file_tree::drag::DraggedFile;
pub use crate::ui::file_tree::model::{
    DIR_RENDER_CAP, DIR_RENDER_STEP, Deco, DirEntry, TreeState, flat_row_index, join, same_path,
    visible_order,
};

use crate::ui::file_tree::header::tree_header;
use crate::ui::file_tree::rows::rows;

use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::rgba;
use crate::probe::registry::probe_area;
use crate::ui::icon::codicon;

/// Дерево воркспейса: FileTreeHeader (title + тулбар) + скролл-тело с
/// корневой папкой ОТДЕЛЬНОЙ разворачиваемой строкой (depth 0) и вложенными.
/// `panel_key` различает id элементов между панелями.
#[allow(clippy::too_many_arguments)]
pub fn file_tree(
    workspace: Option<&str>,
    tree: &TreeState,
    // `treeAllCollapsed` — глиф/лейбл кнопки Collapse ↔ Expand
    all_collapsed: bool,
    // Активный файл редактора есть → Locate доступен
    has_editor_file: bool,
    p: &Palette,
    panel_key: &'static str,
    // Ширина панели в логических px — бюджет усечения имён. ПАРАМЕТРОМ, а не из
    // probe-реестра: на переигранном кадре prepaint канваса не исполняется, и
    // панель считала бы бюджет по позапрошлой ширине (ревью компонентизации).
    width: f32,
    on_toggle: impl Fn(String) + Clone + 'static,
    on_open_file: impl Fn(String) + Clone + 'static,
    on_menu: impl Fn(String, bool, f32, f32) + Clone + 'static,
    on_refresh: impl Fn() + 'static,
    on_collapse_all: impl Fn() + 'static,
    on_locate: impl Fn() + 'static,
    on_show_more: impl Fn(String) + Clone + 'static,
    on_select: impl Fn(String, bool, bool) + Clone + 'static,
) -> AnyElement {
    // FileTreeHeader рисуется ВСЕГДА, в том числе в пустом состоянии:
    // титул «PROJECT», кнопки disabled (`FileTreeHeader.tsx:22-76`).
    let header = tree_header(
        workspace,
        tree,
        all_collapsed,
        has_editor_file,
        p,
        on_locate,
        on_collapse_all,
        on_refresh,
    );
    let Some(root) = workspace else {
        return div()
            .id(panel_key)
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(
                // `.empty`: flex 1, центр, gap space-2, padding space-5
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(m::SPACE_2))
                    .p(px(m::SPACE_5))
                    .text_center()
                    .text_color(rgba(p.text_muted))
                    // `.emptyIcon{font-size:32px}` стоит на САМОМ `<i class=
                    // "codicon codicon-folder emptyIcon">` (0,1,0) и проигрывает
                    // вендорной базе (0,2,0) → фактически 16 (ревью ц.13)
                    .child(codicon("\u{ea83}", 16.0).text_color(rgba(p.text_disabled)))
                    .child(
                        div()
                            .text_size(px(m::FS_SM))
                            .child("No active session with a folder."),
                    )
                    .child(
                        div()
                            .text_size(px(m::FS_SM))
                            .child("Pick a session in Projects, or start one with a folder."),
                    ),
            )
            .child(probe_area(panel_key))
            .into_any_element();
    };

    // Имя корневой папки — для её строки и иконки
    let title = root
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(root)
        .to_string();

    // Корневая папка = отдельная строка depth 0 (chevron + folder icon + имя),
    // разворачиваемая. Дети рендерятся при expanded.
    let root_expanded = tree.expanded.contains(root);
    crate::probe::registry::record_tree_selection(
        tree.selected.iter().cloned().collect(),
        tree.anchor.clone(),
    );
    // Скролл-тело ниже: отдельный контейнер (flex_1) — скроллбар
    // пересчитывается при ресайзе панели.
    let root_row = crate::ui::file_tree::root_row::root_row(
        root,
        &title,
        root_expanded,
        tree,
        p,
        &on_toggle,
        &on_menu,
        &on_select,
        panel_key,
        width,
    );
    let mut scroll_body = div()
        .id(panel_key)
        .relative()
        .flex_1()
        .min_h(px(0.))
        .flex()
        .flex_col()
        .text_size(px(m::FS_SM))
        .overflow_y_scrollbar_with(&tree.scroll)
        // Зазор бар↔правая кромка панели (бар прилипал к краю — юзер).
        .scrollbar_inset_right(3.0)
        .px(px(6.0))
        .pt(px(4.0))
        .pb(px(8.0))
        // RMB по ПУСТОЙ области тела открывает меню корня (строки гасят
        // событие своим обработчиком) — `FileTreeView.tsx:61-66`
        .on_mouse_down(gpui::MouseButton::Right, {
            let cb = on_menu.clone();
            let root = root.to_string();
            move |ev: &gpui::MouseDownEvent, _, _| {
                cb(
                    root.clone(),
                    true,
                    f32::from(ev.position.x),
                    f32::from(ev.position.y),
                );
            }
        })
        .child(probe_area(panel_key))
        .child(root_row);

    if root_expanded {
        let mut out = Vec::new();
        rows(
            &mut out,
            tree,
            root,
            1.0,
            p,
            &on_toggle,
            &on_open_file,
            &on_menu,
            &on_show_more,
            &on_select,
            panel_key,
            width,
        );
        // ВРЕМЕННАЯ диагностика: сколько строк дерева строится за кадр.
        crate::web::rows_built(out.len() as u32);
        scroll_body = scroll_body.children(out);
    }

    div()
        .relative()
        // `.root { flex: 1 }` (`FileTreeView.module.css:1-6`). ЗАМЕНА на
        // `flex_1` здесь ЛОМАЕТ раскладку: родитель дерева не flex-колонка с
        // ограниченной высотой, и тело вытягивается по контенту (замер:
        // 2293 px против 833 у колонки). `size_full` даёт тот же результат,
        // что `flex: 1` у оригинала, потому что высоту задаёт родитель
        // (проверено ц.29)
        .size_full()
        .flex()
        .flex_col()
        .min_w(px(0.))
        .min_h(px(0.))
        // Поэлементный кроп parity/shots.py
        .child(probe_area("file-tree"))
        .child(header)
        .child(scroll_body)
        .into_any_element()
}
