//! Строки дерева: отрисовка узлов с отступами и состояниями.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::ui::file_tree::model::{DIR_RENDER_STEP, DirEntry, TreeState, cap_for, join};
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rows(
    out: &mut Vec<AnyElement>,
    tree: &TreeState,
    dir: &str,
    depth: f32,
    p: &Palette,
    on_toggle: &(impl Fn(String) + Clone + 'static),
    on_open_file: &(impl Fn(String) + Clone + 'static),
    on_menu: &(impl Fn(String, bool, f32, f32) + Clone + 'static),
    on_show_more: &(impl Fn(String) + Clone + 'static),
    on_select: &(impl Fn(String, bool, bool) + Clone + 'static),
    panel_key: &'static str,
    width: f32,
) {
    // Ширина панели прошлого кадра — бюджет усечения меток
    let panel_w = width;
    let Some(entries) = tree.cache.get(dir) else {
        // `.loading` — листинг ещё не пришёл (оригинал FileTreeView.tsx:201)
        out.push(
            div()
                .pl(px((depth) * 12.0 + 8.0))
                .py(px(2.0))
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                // ПКМ по вспомогательной строке НЕ открывает меню корня:
                // оригинал требует `e.target === e.currentTarget` (ревью ц.14)
                .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child("Loading…")
                .into_any_element(),
        );
        return;
    };
    if entries.is_empty() {
        // `.emptyChild` — раскрытая пустая папка (оригинал :204)
        out.push(
            div()
                .pl(px((depth) * 12.0 + 8.0))
                .py(px(2.0))
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child("(empty)")
                .into_any_element(),
        );
        return;
    }
    // «Show N more»: длинные листинги усечены до капа (100 + шаги по 200)
    let cap = cap_for(tree, dir);
    let capped = entries.len() > cap;
    let visible: &[DirEntry] = if capped { &entries[..cap] } else { entries };
    let mut dir_marked = false;
    let mut file_marked = false;
    for e in visible {
        let path = join(dir, &e.name);
        let (dir_seen, file_seen) = (dir_marked, file_marked);
        if e.is_dir {
            dir_marked = true;
        } else {
            file_marked = true;
        }
        let (dir_marked, file_marked) = (dir_seen, file_seen);
        let expanded = crate::ui::file_tree::row::row_of(
            out,
            e,
            &path,
            tree,
            depth,
            panel_w,
            dir_marked,
            file_marked,
            p,
            on_toggle,
            on_open_file,
            on_menu,
            on_select,
            panel_key,
        );
        if expanded {
            rows(
                out,
                tree,
                &path,
                depth + 1.0,
                p,
                on_toggle,
                on_open_file,
                on_menu,
                on_show_more,
                on_select,
                panel_key,
                width,
            );
        }
    }
    if capped {
        let hidden = entries.len() - cap;
        let more = hidden.min(DIR_RENDER_STEP);
        let cb = on_show_more.clone();
        let dir_owned = dir.to_string();
        let hover_bg = {
            let mut c = rgba(p.bg_surface);
            c.a = 0.55;
            c
        };
        let hover_fg = rgba(p.text_primary);
        out.push(
            div()
                .id(SharedString::from(format!("{panel_key}:more:{dir}")))
                .group("ft-showmore")
                .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| {
                    cx.stop_propagation();
                })
                .flex()
                .items_center()
                .gap(px(6.0))
                // `indentPx(depth + 1)` оригинала считает от глубины РОДИТЕЛЯ, а наш
                // `depth` уже детский → без +1 (ревью ц.8)
                .pl(px(depth * 12.0 + 8.0))
                .py(px(3.0))
                // `.showMore` (FileTreeView.module.css:164-178) скругления НЕ
                // задаёт — у нас был лишний RADIUS_XS (ревью ц.10)
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    move |_, _, _| cb(dir_owned.clone()),
                )
                // `<i class="codicon codicon-ellipsis">` без модульного класса —
                // база 16px, а не 13 (ревью ц.13)
                .child(
                    // `.showMore:hover { color: text-primary }` красит и глиф
                    // (ревью ц.21)
                    codicon("\u{ea7c}", 16.0)
                        .group_hover("ft-showmore", move |st| st.text_color(hover_fg)),
                )
                .child(format!("Show {more} more ({hidden} hidden)"))
                .into_any_element(),
        );
    }
}
