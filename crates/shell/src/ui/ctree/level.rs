//! Уровень contributed-дерева: рекурсивная отрисовка узлов.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::host::events::TreeEvent;
use crate::host_link::ShellEvent;
use crate::ui::contributed_tree::TREE_CHILD_CAP;
use crate::ui::ctree::model::{DraggedTreeNode, TreeDragGhost};
use crate::ui::ctree::nodes::{indent, muted_line};
use crate::ui::ctree::types::{NONE, TreeViewState};
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

#[allow(clippy::too_many_arguments)]
pub(crate) fn level(
    out: &mut Vec<AnyElement>,
    view: &str,
    st: &TreeViewState,
    parent: &str,
    depth: f32,
    tx: &Sender<ShellEvent>,
    p: &Palette,
    // handle из `treeReveal` → его индекс в плоском списке строк
    reveal: Option<&str>,
    reveal_ix: &mut Option<usize>,
) {
    // Ширина тела вью прошлого кадра — бюджет усечения меток
    let panel_w = crate::probe::registry::bounds_of(view)
        .map(|[_, _, w, _]| w)
        .unwrap_or(240.0);
    let Some(slot) = st.levels.get(parent) else {
        out.push(muted_line("Loading…".into(), depth, p));
        return;
    };
    let Some(nodes) = slot else {
        out.push(muted_line("Loading…".into(), depth, p));
        return;
    };
    if nodes.is_empty() {
        if depth == 0.0 {
            out.push(muted_line("(empty)".into(), depth, p));
        }
        return;
    }
    let capped = nodes.len() > TREE_CHILD_CAP;
    let shown = if capped {
        &nodes[..TREE_CHILD_CAP]
    } else {
        &nodes[..]
    };
    for node in shown {
        if reveal == Some(node.handle.as_str()) {
            *reveal_ix = Some(out.len());
        }
        let expandable = node.collapsible != NONE;
        let expanded = expandable && st.is_expanded(node);
        let selected = st.selected.as_deref() == Some(node.handle.as_str());
        let hover_bg = {
            let mut c = rgba(p.bg_surface);
            c.a = 0.55;
            c
        };
        let hover_fg = rgba(p.text_primary);
        let row_group = SharedString::from(format!("ctrow:{}:{}", view, node.handle));
        let mut row = div()
            .group(row_group.clone())
            .id(SharedString::from(format!("tv:{view}:{}", node.handle)))
            .flex()
            .items_center()
            .gap(px(6.0))
            .w_full()
            .pl(px(indent(depth)))
            .pr(px(m::SPACE_2))
            .h(px(22.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .border_1()
            .border_color(gpui::transparent_black())
            .rounded(px(m::RADIUS_XS))
            .text_size(px(m::FS_SM))
            .text_color(rgba(p.text_secondary))
            .cursor_pointer()
            .when(!selected, |r| {
                r.hover(move |s| s.bg(hover_bg).text_color(hover_fg))
            })
            .tooltip(crate::ui::tooltip::tooltip(
                node.tooltip.clone().unwrap_or_else(|| node.label.clone()),
            ));
        if selected {
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
        // Клик: тоггл раскрытия + выделение + команда узла (паритет VS Code)
        {
            let tx = tx.clone();
            let view = view.to_string();
            let handle = node.handle.clone();
            let command = node.command.clone();
            let next = !expanded;
            // `onClick` оригинала, а не mouse-down: перетаскивание строки не
            // должно её раскрывать и выделять (gpui гасит pending-click, как
            // только начался драг, — `div.rs:1569-1576`)
            // Второй клик двойного щелчка в браузере тоже даёт `click` —
            // фильтровать его нельзя, иначе двойной щелчок не свернёт узел
            row = row.on_click(move |_, _, _| {
                let _ = tx.try_send(ShellEvent::Tree(TreeEvent::TreeClick {
                    view: view.clone(),
                    handle: handle.clone(),
                    expandable,
                    expanded: next,
                    command: command.clone(),
                }));
            });
        }
        // DnD: строки таскаются только когда вью зарегистрировала контроллер
        // (`draggable={dndEnabled}`); drop по строке шлёт цель хосту
        if st.dnd {
            let label = node.label.clone();
            let (dv, dh) = (view.to_string(), node.handle.clone());
            let dtx = tx.clone();
            row = row.on_drag(DraggedTreeNode, move |_, _, _, cx| {
                let _ = dtx.try_send(ShellEvent::Tree(TreeEvent::TreeDragStart {
                    view: dv.clone(),
                    handle: dh.clone(),
                }));
                let label = label.clone();
                cx.new(|_| TreeDragGhost { label })
            });
            let accent = rgba(p.accent_primary);
            row = row.drag_over::<DraggedTreeNode>(move |stl, _, _, _| {
                let mut fill = accent;
                fill.a = 0.22;
                stl.bg(fill).border_color(accent)
            });
            let (v2, h2) = (view.to_string(), node.handle.clone());
            let tx2 = tx.clone();
            row = row.on_drop(move |_: &DraggedTreeNode, _, cx| {
                cx.stop_propagation();
                let _ = tx2.try_send(ShellEvent::Tree(TreeEvent::TreeDrop {
                    view: v2.clone(),
                    handle: h2.clone(),
                }));
            });
        }
        let row = crate::ui::ctree::row_body::row_body(
            row, node, view, expandable, expanded, selected, row_group, panel_w, depth, tx, p,
        );
        out.push(
            crate::ui::focus_ring::focusable(
                row,
                &format!("ctrow:{view}:{}", node.handle),
                m::RADIUS_XS,
                rgba(p.accent_primary),
            )
            .into_any_element(),
        );
        if expanded {
            level(
                out,
                view,
                st,
                &node.handle,
                depth + 1.0,
                tx,
                p,
                reveal,
                reveal_ix,
            );
        }
    }
    if capped {
        out.push(muted_line(
            format!("… {} more", nodes.len() - TREE_CHILD_CAP),
            depth,
            p,
        ));
    }
}
