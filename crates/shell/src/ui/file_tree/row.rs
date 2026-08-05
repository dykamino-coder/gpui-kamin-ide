//! Одна строка дерева файлов: иконка, метка, состояния, перетаскивание.
//!
//! Тело цикла вынесено из `rows` дословно (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::probe::registry::probe_area;
use crate::ui::file_tree::drag::{DraggedFile, FileDragGhost};
use crate::ui::file_tree::model::{DirEntry, TreeState, same_path};
use crate::ui::file_tree::row_parts as parts;

use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Одна строка дерева. `dir_seen`/`file_seen` — были ли ДО этой записи
/// встречены папка и файл (разделитель групп рисуется по ним).
#[allow(clippy::too_many_arguments)]
pub(crate) fn row_of(
    out: &mut Vec<AnyElement>,
    e: &DirEntry,
    path: &str,
    tree: &TreeState,
    depth: f32,
    panel_w: f32,
    dir_marked: bool,
    file_marked: bool,
    p: &Palette,
    on_toggle: &(impl Fn(String) + Clone + 'static),
    on_open_file: &(impl Fn(String) + Clone + 'static),
    on_menu: &(impl Fn(String, bool, f32, f32) + Clone + 'static),
    on_select: &(impl Fn(String, bool, bool) + Clone + 'static),
    panel_key: &'static str,
) -> bool {
    let path = path.to_string();
    let expanded = e.is_dir && tree.expanded.contains(&path);
    // .row:hover bg-surface 55% (FileTreeView.module.css:87)
    let hover_bg = {
        let mut c = rgba(p.bg_surface);
        c.a = 0.55;
        c
    };
    let hover_fg = rgba(p.text_primary);
    let is_selected = tree.selected.iter().any(|s| same_path(s, &path));
    let row_group = SharedString::from(format!("ftrow:{panel_key}:{path}"));
    let mut row = div()
        .id(SharedString::from(format!("{panel_key}:{path}")))
        .relative()
        // Поэлементный кроп: регион у ПЕРВОЙ строки корневого уровня
        .when(out.is_empty(), |r| r.child(probe_area("file-tree-row")))
        // Досье 94 и 95 — РАЗНЫЕ элементы (папка и файл), и общий регион
        // строки давал им один кроп на двоих (ревью ц.26). Метим ПЕРВУЮ
        // папку и ПЕРВЫЙ файл уровня отдельно
        .when(e.is_dir && !dir_marked, |r| {
            r.child(probe_area("file-tree-folder-row"))
        })
        .when(!e.is_dir && !file_marked, |r| {
            r.child(probe_area("file-tree-file-row"))
        })
        .flex()
        .items_center()
        .gap(px(6.0)) // .row gap 6 (css:65)
        // FileTreeView: indent = depth*12 + 8 (plan/23)
        .pl(px(depth * 12.0 + 8.0))
        .pr(px(m::SPACE_2))
        // .row height 22 + `border: 1px solid transparent` — резерв, чтобы
        // accent-бордер выделения не сдвигал лейаут
        .h(px(22.0))
        // `.row { white-space: nowrap; overflow: hidden }` — было только у
        // лейбла, длинный бейдж/описание вылезали за строку (ревью ц.13)
        .whitespace_nowrap()
        .overflow_hidden()
        .border_1()
        .border_color(gpui::transparent_black())
        .rounded(px(m::RADIUS_XS))
        .text_color(rgba(p.text_secondary))
        .cursor_pointer()
        // `.rowSelected:hover` сохраняет accent-градиент → ховер только
        // у НЕ выделенной строки
        .group(row_group.clone())
        .when(!is_selected, |r| {
            r.hover(move |s| s.bg(hover_bg).text_color(hover_fg))
        });
    // `.dropTarget` (`FileTreeView.module.css:97-102`): папка под
    // перетаскиваемым файлом — accent 22 % + рамка accent (offset −1,
    // у нас рамка строки уже зарезервирована). Раньше состояния не было.
    if e.is_dir {
        let accent = rgba(p.accent_primary);
        row = row.drag_over::<DraggedFile>(move |st, _, _, _| {
            let mut fill = accent;
            fill.a = 0.22;
            st.bg(fill).border_color(accent)
        });
        // Drop внутреннего драга на папку = MOVE набора (plan/99 п.34);
        // guard от «в саму себя» и no-op в свой каталог — в обработчике.
        // tx в row_of не пробрасывается (только колбэки) — глобальный
        // event_tx, как у probe.
        let dest = path.clone();
        row = row.on_drop(move |df: &DraggedFile, _, _| {
            if let Some(tx) = crate::host_link::event_tx() {
                let _ = tx.try_send(crate::host_link::ShellEvent::Ed(
                    crate::host::events::EdEvent::FsDropMove {
                        dest: dest.clone(),
                        paths: df.paths.clone(),
                        copy: false,
                    },
                ));
            }
        });
        // Внешний drop из ОС на папку = COPY внутрь (та же подсветка цели).
        let accent2 = rgba(p.accent_primary);
        row = row.drag_over::<gpui::ExternalPaths>(move |st, _, _, _| {
            let mut fill = accent2;
            fill.a = 0.22;
            st.bg(fill).border_color(accent2)
        });
        let dest_ext = path.clone();
        row = row.on_drop(move |ext: &gpui::ExternalPaths, _, _| {
            if let Some(tx) = crate::host_link::event_tx() {
                let paths: Vec<String> = ext
                    .paths()
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect();
                let _ = tx.try_send(crate::host_link::ShellEvent::Ed(
                    crate::host::events::EdEvent::FsDropMove {
                        dest: dest_ext.clone(),
                        paths,
                        copy: true,
                    },
                ));
            }
        });
    }
    if is_selected {
        // .rowSelected: градиент accent 26→14% + бордер 45% (css:89-94)
        let mut g1 = rgba(p.accent_primary);
        g1.a = 0.26;
        let mut g2 = rgba(p.accent_primary);
        g2.a = 0.14;
        let mut bc = rgba(p.accent_primary);
        bc.a = 0.45;
        row = row
            .bg(gpui::linear_gradient(
                90.,
                gpui::linear_color_stop(g1, 0.),
                gpui::linear_color_stop(g2, 1.),
            ))
            .border_color(bc)
            .text_color(rgba(p.text_primary));
    }
    // `onClick` оригинала, а не mouse-down: начало драга не должно
    // раскрывать папку и открывать файл — в браузере после драга `click`
    // не приходит, gpui гасит pending-click так же (ревью ц.23)
    {
        let cb_dir = on_toggle.clone();
        let cb_file = on_open_file.clone();
        let sel_cb = on_select.clone();
        let path_for_click = path.clone();
        let is_dir = e.is_dir;
        row = row.on_click(move |ev: &gpui::ClickEvent, _, _| {
            let m = ev.modifiers();
            sel_cb(path_for_click.clone(), m.control, m.shift);
            // Ctrl/Shift — только выделение, без раскрытия и открытия
            // (`FileTreeView.tsx:184`)
            if m.control || m.shift {
                return;
            }
            if is_dir {
                cb_dir(path_for_click.clone());
            } else {
                cb_file(path_for_click.clone());
            }
        });
    }
    // Внутренний drag: `draggable={depth > 0}` — тащатся и файлы, и папки
    {
        let name = e.name.clone();
        // `dragPaths(path)`: строка внутри мультивыбора тащит весь набор
        let paths: Vec<String> =
            if tree.selected.iter().any(|x| same_path(x, &path)) && tree.selected.len() > 1 {
                tree.selected.iter().cloned().collect()
            } else {
                vec![path.clone()]
            };
        let ghost_extra = paths.len().saturating_sub(1);
        row = row.on_drag(DraggedFile { paths }, move |_, _, _, cx| {
            let name = if ghost_extra > 0 {
                format!("{name} +{ghost_extra}")
            } else {
                name.clone()
            };
            cx.new(|_| FileDragGhost { name })
        });
    }
    // RMB → контекст-меню узла (New/Cut/Copy/Paste/Rename/Delete/CopyPath)
    row = crate::ui::file_tree::row_menu::with_menu(row, &path, e.is_dir, on_menu);
    let loading = e.is_dir && tree.loading.contains(&path);
    row = row
        .child(parts::chevron(
            e,
            &path,
            expanded,
            loading,
            is_selected,
            panel_key,
            p,
        ))
        .child(parts::glyph(e, expanded))
        .child(parts::label(
            e,
            &path,
            tree,
            depth,
            panel_w,
            panel_key,
            row_group.clone(),
            hover_fg,
            p,
        ))
        .children(parts::badge(tree, &path, panel_key, row_group, hover_fg, p));
    // `.flash` — `treeFlash 0.9s ease-out 1`: accent 40 % → transparent
    let row = match &tree.flash {
        Some((fp, seq)) if *fp == path => {
            use gpui::AnimationExt as _;
            let accent = rgba(p.accent_primary);
            row.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .rounded(px(m::RADIUS_XS))
                    .with_animation(
                        SharedString::from(format!("tree-flash-{seq}")),
                        gpui::Animation::new(std::time::Duration::from_millis(900)),
                        move |d, delta| {
                            let mut c = accent;
                            c.a = 0.40 * (1.0 - delta);
                            d.bg(c)
                        },
                    ),
            )
        }
        _ => row,
    };
    out.push(
        crate::ui::focus_ring::focusable(
            row,
            &format!("frow:{path}"),
            m::RADIUS_XS,
            rgba(p.accent_primary),
        )
        .into_any_element(),
    );
    expanded
}
