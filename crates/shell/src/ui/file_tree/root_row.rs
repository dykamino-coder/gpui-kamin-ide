//! Строка корня дерева файлов: имя воркспейса, раскрытие, меню.
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::ui::file_tree::drag::DraggedFile;
use crate::ui::file_tree::model::{TreeState, deco_color, same_path};
use crate::ui::icon::{CHEVRON_DOWN, CHEVRON_RIGHT, codicon};
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

#[allow(clippy::too_many_arguments)]
pub(crate) fn root_row(
    root: &str,
    title: &str,
    root_expanded: bool,
    tree: &TreeState,
    p: &Palette,
    on_toggle: &(impl Fn(String) + Clone + 'static),
    on_menu: &(impl Fn(String, bool, f32, f32) + Clone + 'static),
    on_select: &(impl Fn(String, bool, bool) + Clone + 'static),
    panel_key: &'static str,
    width: f32,
) -> AnyElement {
    {
        // Бюджет усечения имени корня — та же ширина панели, что у узлов
        let panel_w = width;
        let hover_bg = {
            let mut c = rgba(p.bg_surface);
            c.a = 0.55;
            c
        };
        let cb = on_toggle.clone();
        let sel_cb = on_select.clone();
        let root_owned = root.to_string();
        let root_selected = tree.selected.iter().any(|x| same_path(x, root));
        let root_loading = tree.loading.contains(root);
        // Декорация корня: у оригинала корень — обычный `FolderNode`, поэтому
        // цвет лейбла, подсказка и бейдж у него ровно те же (ревью ц.24)
        let root_deco = tree.deco.get(root).and_then(|d| d.as_ref());
        let root_deco_color = root_deco
            .and_then(|d| d.color.as_deref())
            .map(|id| rgba(deco_color(id, p)));
        let root_tooltip = root_deco
            .and_then(|d| d.tooltip.clone())
            .unwrap_or_else(|| root.to_string());
        let root_badge = root_deco.and_then(|d| d.badge.clone());
        // Ховер строки красит лейбл — как у узлов (ревью ц.23)
        let root_group = SharedString::from(format!("{panel_key}:rootgrp"));
        let accent = rgba(p.accent_primary);
        let row = div()
            .id(SharedString::from(format!("{panel_key}:root")))
            .group(root_group.clone())
            .flex()
            .items_center()
            .gap(px(6.0))
            .pl(px(8.0))
            .pr(px(m::SPACE_2))
            .h(px(22.0))
            // Корень в оригинале — обычный `.row`: `white-space: nowrap` +
            // `overflow: hidden` (ревью ц.14)
            .whitespace_nowrap()
            .overflow_hidden()
            .border_1()
            .border_color(gpui::transparent_black())
            .rounded(px(m::RADIUS_XS))
            .text_color(rgba(p.text_secondary))
            .cursor_pointer()
            .when(!root_selected, |r| {
                r.hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
            })
            .when(root_selected, |r| {
                // `.rowSelected` — тот же градиент и рамка, что у узлов
                let mut g1 = rgba(p.accent_primary);
                g1.a = 0.26;
                let mut g2 = rgba(p.accent_primary);
                g2.a = 0.14;
                let mut bc = rgba(p.accent_primary);
                bc.a = 0.45;
                r.bg(gpui::linear_gradient(
                    90.,
                    gpui::linear_color_stop(g1, 0.),
                    gpui::linear_color_stop(g2, 1.),
                ))
                .border_color(bc)
                .text_color(rgba(p.text_primary))
            })
            // `onClick`, а не mouse-down: корень — тот же `FolderNode`
            // (`FileTreeView.tsx:184`), и начало драга не должно его сворачивать.
            // Узлы починили в ц.23, корень тогда выпал (ревью ц.25)
            .on_click(move |ev: &gpui::ClickEvent, _, _| {
                let m = ev.modifiers();
                // `applyClickSelection` вернул true (был Ctrl) → раскрытие
                // НЕ трогаем
                sel_cb(root_owned.clone(), m.control, m.shift);
                if !m.control && !m.shift {
                    cb(root_owned.clone());
                }
            })
            .on_mouse_down(gpui::MouseButton::Right, {
                let cb = on_menu.clone();
                let root = root.to_string();
                move |ev: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    cb(
                        root.clone(),
                        true,
                        f32::from(ev.position.x),
                        f32::from(ev.position.y),
                    );
                }
            })
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(16.0))
                    .flex()
                    .justify_center()
                    // `.rowSelected .chevron { color: inherit }` — у выделенного
                    // корня шеврон наследует цвет строки (ревью ц.13)
                    .when(!root_selected, |d| d.text_color(rgba(p.text_muted)))
                    .map(|d| {
                        // Корень в оригинале — тот же `FolderNode` (depth 0),
                        // значит и `codicon-loading codicon-modifier-spin`
                        // у него есть (ревью ц.24)
                        if root_loading {
                            d.child(crate::ui::icon::spinner(
                                SharedString::from(format!("spin:{panel_key}:root")),
                                16.0,
                                if root_selected {
                                    rgba(p.text_primary)
                                } else {
                                    rgba(p.text_muted)
                                },
                            ))
                        } else {
                            d.child(codicon(
                                if root_expanded {
                                    CHEVRON_DOWN
                                } else {
                                    CHEVRON_RIGHT
                                },
                                16.0,
                            ))
                        }
                    }),
            )
            .child(
                crate::icon_theme::folder_img(title, root_expanded, true)
                    .flex_shrink_0()
                    .w(px(16.0))
                    .h(px(16.0)),
            )
            .child(
                div()
                    .id(SharedString::from(format!("lbl:{panel_key}:root")))
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .group_hover(root_group.clone(), {
                        let tp = rgba(p.text_primary);
                        move |st| st.text_color(tp)
                    })
                    // `data-tooltip={deco?.tooltip ?? path}` — как у узлов
                    .tooltip(crate::ui::tooltip::tooltip(root_tooltip))
                    .when_some(root_deco_color, |d, c| d.text_color(c))
                    // Тот же `fit_approx`, что у узлов: движок «…» не рисует
                    // Бюджет тот же, что у узлов: минус индент depth 0 (=8),
                    // шеврон 16, иконка 16, два гэпа 6 и pr 8 (ревью ц.25)
                    .child(crate::ui::text_fit::fit_approx(
                        title,
                        panel_w - 8.0 - 52.0,
                        m::FS_SM,
                    )),
            )
            // Бейдж декорации корня — тот же рецепт, что у узлов
            .children(root_badge.map(|badge| {
                div()
                    .id(SharedString::from(format!("badge:{panel_key}:root")))
                    .flex_shrink_0()
                    .ml_auto()
                    .pl(px(6.0))
                    .text_size(px(m::FS_XS))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .when(root_deco_color.is_none(), |d| {
                        let tp = rgba(p.text_primary);
                        d.group_hover(root_group.clone(), move |st| st.text_color(tp))
                    })
                    .when_some(root_deco_color, |d, c| d.text_color(c))
                    .child(badge)
            }));
        // `.dropTarget` корня: в оригинале его получает ЛЮБАЯ папка, включая
        // depth 0 (`FileTreeView.tsx:175`)
        let row = row.drag_over::<DraggedFile>(move |st, _, _, _| {
            let mut fill = accent;
            fill.a = 0.22;
            st.bg(fill).border_color(accent)
        });
        // `.flash` у корня: он такой же `FolderNode`, Locate по нему тоже
        // подсвечивает строку (ревью ц.24)
        let row = match &tree.flash {
            Some((fp, seq)) if same_path(fp, root) => {
                use gpui::AnimationExt as _;
                row.relative().child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .rounded(px(m::RADIUS_XS))
                        .with_animation(
                            SharedString::from(format!("tree-flash-root-{seq}")),
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
        crate::ui::focus_ring::focusable(
            row,
            &format!("frow:{root}"),
            m::RADIUS_XS,
            rgba(p.accent_primary),
        )
        .into_any_element()
    }
}
