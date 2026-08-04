//! Один таб редактора: имя, пин, «грязный» маркер, закрытие, drag.
//!
//! Тело цикла вынесено из `editor_tabs_bar` как есть (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::EdEvent;
use crate::host_link::ShellEvent;
use crate::ui::editor::tab_name::base_name;
use gpui::prelude::*;
use gpui::{SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

#[allow(clippy::too_many_arguments)]
pub(crate) fn tab_item(
    strip: gpui::Stateful<gpui::Div>,
    i: usize,
    tabs: &[(String, bool, bool)],
    active: usize,
    _widths: &[f32],
    dragging: Option<usize>,
    drag_over: Option<usize>,
    p: &Palette,
    tx: &Sender<ShellEvent>,
) -> gpui::Stateful<gpui::Div> {
    let mut strip = strip;
    let (path, dirty, pinned) = &tabs[i];
    let is_active = i == active;
    let name = base_name(path);
    let hover_bg = tint(rgba(p.bg_surface), 0.5);
    let group = SharedString::from(format!("ftab-g-{i}"));
    let mut tab = div()
        .id(SharedString::from(format!("ftab-{i}")))
        // Регион ПЕРВОГО таба — досье 111; полоса целиком это 109/110
        .when(i == 0, |t| {
            t.relative()
                .child(crate::probe::registry::probe_area("file-viewer-tab"))
        })
        .group(group.clone())
        // якорь для абсолютного `.dropIndicator`
        .relative()
        .flex()
        .items_center()
        .gap(px(6.0))
        .h(px(24.0))
        // `.tab { flex-shrink: 0 }` — при переполнении табы НЕ сжимаются,
        // лишние уходят в «▾» (ревью ц.13)
        .flex_shrink_0()
        // .tab padding 4/6/4/10 (ревью ц.1: было 8/4)
        .pl(px(10.0))
        .pr(px(6.0))
        .rounded(px(m::RADIUS_SM))
        .text_size(px(11.0))
        // `letter-spacing: 0.02em` (`FileViewerTabs.module.css:95`)
        .letter_spacing(px(11.0 * 0.02))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(rgba(p.text_secondary))
        .cursor_pointer()
        // `.tabDragging { opacity: 0.3 }` — исходный таб гаснет, пока
        // его тащат (`FileViewerTabs.module.css:159`, ревью ц.17)
        .when(dragging == Some(i), |t| t.opacity(0.3))
        .tooltip(crate::ui::tooltip::tooltip(path.clone()))
        // Нажатие = кандидат drag-reorder; select случится на mouse-up
        // без движения (root разруливает порог 4px)
        .on_mouse_down(gpui::MouseButton::Left, {
            let tx = tx.clone();
            move |e: &gpui::MouseDownEvent, _, _| {
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::TabPress(
                    i,
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                )));
            }
        })
        // middle-click закрывает (как оригинал)
        .on_mouse_down(gpui::MouseButton::Middle, {
            let tx = tx.clone();
            move |_, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseEditorTab(i)));
            }
        })
        // RMB → меню таба (Close-действия + файловые из дерева)
        .on_mouse_down(gpui::MouseButton::Right, {
            let tx = tx.clone();
            let path = path.clone();
            move |e: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::OpenEditorTabMenu(
                    i,
                    path.clone(),
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                )));
            }
        })
        // Порядок 1:1: pinIcon → tabIcon → label; пин = codicon-pinned 11 op .7
        .when(*pinned, |t| {
            t.child(
                crate::ui::icon::codicon("\u{eba0}", 16.0)
                    .flex_shrink_0()
                    .opacity(0.7),
            )
        })
        .child(
            crate::icon_theme::file_img(&name)
                .flex_shrink_0()
                .w(px(14.0))
                .h(px(14.0)),
        )
        .child({
            // `.tab:hover { color: text-primary }` красит подпись;
            // собственный `.hover()` до дочернего текста не доходит
            // (замеры ц.20) — только через группу
            let hover_fg = rgba(p.text_primary);
            let g = group.clone();
            // `.label { white-space: nowrap }` (`FileViewerTabs.module.css:123`)
            div()
                .whitespace_nowrap()
                .child(name)
                .when(!is_active, move |d| {
                    d.group_hover(g, move |st| st.text_color(hover_fg))
                })
        });
    if is_active {
        tab = tab
            .bg(tint(rgba(p.accent_primary), 0.16))
            .text_color(rgba(p.text_primary));
    } else {
        tab = tab.hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)));
    }
    // `.dropIndicator`: абсолютная полоса 2px accent, top/bottom 5, r1 —
    // НЕ левая рамка таба (та съедала 2px ширины у соседей)
    // Индикатор — в ЦЕНТРЕ ЗАЗОРА (`before.left − strip.left − GAP_HALF`,
    // гэп 4 → половина 2), а не по кромке таба (ревью ц.23)
    // `if (from >= 0 && (over === from || over === from + 1)) return
    // { over: -1 }` (`FileViewerTabs.tsx:117`): на позициях, где перенос
    // ничего не меняет, индикатор ГАСНЕТ. Мы рисовали его и там
    // (ревью ц.26)
    let noop_drop = dragging.is_some_and(|from| i == from || i == from + 1);
    if drag_over == Some(i) && !noop_drop {
        tab = tab.child(
            div()
                .absolute()
                .left(px(-2.0))
                .top(px(5.0))
                .bottom(px(5.0))
                .w(px(2.0))
                .rounded(px(1.0))
                .bg(rgba(p.accent_primary)),
        );
    }
    // dirty-точка И close × — ОБА (ревью ц.1: раньше XOR)
    if *dirty {
        // `.dirty::before { content: "●"; font-size: 10px }` — глиф, а не
        // нарисованный квадрат 6×6 (ревью ц.11)
        tab = tab.child(
            div()
                .flex_shrink_0()
                .text_size(px(10.0))
                // `.dirty { line-height: 1 }` — без него глиф тянет
                // строчную высоту таба (ревью ц.24)
                .line_height(px(10.0))
                .text_color(rgba(p.accent_orange))
                .child("\u{25cf}"),
        );
    }
    {
        // .close: opacity 0 → .7 на hover таба/активном → 1 + bg-overlay
        // 60% на своём hover; r-xs
        let tx = tx.clone();
        let mut close = div()
            .id(SharedString::from(format!("ftabx-{i}")))
            .w(px(16.0))
            .h(px(16.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(m::RADIUS_XS))
            .hover({
                let hb = tint(rgba(p.bg_overlay), 0.6);
                move |s| s.bg(hb).opacity(1.0)
            })
            // `FileViewerTabs.tsx:188`: у грязного таба подсказка
            // предупреждает о потере правок
            .tooltip(crate::ui::tooltip::tooltip(if *dirty {
                "Discard & close"
            } else {
                "Close"
            }))
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseEditorTab(i)));
            })
            .child(crate::ui::icon::codicon("\u{ea76}", 11.0));
        close = if is_active {
            close.opacity(0.7)
        } else {
            close
                .opacity(0.0)
                .group_hover(group.clone(), |s| s.opacity(0.7))
        };
        tab = tab.child(close);
    }
    strip = strip.child(tab);
    strip
}
