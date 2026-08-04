//! Узлы contributed-дерева: отступ, иконка, чекбокс, строка-заглушка.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::host::events::TreeEvent;
use crate::host_link::ShellEvent;
use crate::ui::contributed_tree::BASE_INDENT_PX;
use crate::ui::contributed_tree::CIRCLE_OUTLINE;
use crate::ui::contributed_tree::FOLDER;
use crate::ui::contributed_tree::{CHECK, INDENT_PX};
use crate::ui::ctree::types::{CHECKED, NONE, TreeNodeDto, UNCHECKED};
use crate::ui::focus_ring::FocusRing;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

pub(crate) fn indent(depth: f32) -> f32 {
    depth * INDENT_PX + BASE_INDENT_PX
}
/// `.loading` / `.emptyChild`: fs-xs, text-muted, py 2 + отступ уровня.
pub(crate) fn muted_line(text: String, depth: f32, p: &Palette) -> AnyElement {
    div()
        .pl(px(indent(depth)))
        .py(px(2.0))
        .text_size(px(m::FS_XS))
        .text_color(rgba(p.text_muted))
        .child(SharedString::from(text))
        .into_any_element()
}
/// Иконка узла: ThemeIcon → codicon; resourceUri → иконка файла/папки;
/// иначе generic circle-outline/folder. Бокс 16×16.
pub(crate) fn node_icon(node: &TreeNodeDto, expanded: bool) -> AnyElement {
    if let Some(name) = &node.codicon {
        // `.icon` задаёт только БОКС 16×16, кегль глифа наследуется от строки
        // (`--fs-sm` 12). Неизвестный ThemeIcon — пустой бокс, как несуществующий
        // класс `codicon-<name>` в оригинале (ревью ц.7).
        return match crate::ui::codicon_map::codicon_by_name(name) {
            // `.icon` кегль не задаёт → база `.codicon` 16 (ревью ц.13)
            Some(glyph) => icon_box(codicon(glyph, 16.0)),
            None => icon_box(div()),
        };
    }
    if let Some(uri) = &node.resource_uri {
        let base = uri.rsplit(['/', '\\']).next().unwrap_or("").to_string();
        let img = if node.collapsible == NONE {
            crate::icon_theme::file_img(&base)
        } else {
            crate::icon_theme::folder_img(&base, expanded, false)
        };
        return img
            .flex_shrink_0()
            .w(px(16.0))
            .h(px(16.0))
            .into_any_element();
    }
    let glyph = if node.collapsible == NONE {
        CIRCLE_OUTLINE
    } else {
        FOLDER
    };
    // Тот же случай: модульный класс кегля не задаёт → 16
    icon_box(codicon(glyph, 16.0))
}
/// `.icon { flex-shrink: 0; width: 16px; height: 16px }` — бокс фиксирован,
/// кегль глифа приходит из строки.
pub(crate) fn icon_box(inner: gpui::Div) -> AnyElement {
    div()
        .flex_shrink_0()
        .w(px(16.0))
        .h(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .child(inner)
        .into_any_element()
}
/// `.treeCheckbox`: 14×14, рамка currentColor, r 3, галка codicon 11.
#[allow(clippy::too_many_arguments)]
pub(crate) fn checkbox(
    view: &str,
    node: &TreeNodeDto,
    state: i64,
    border: gpui::Rgba,
    // `.row:hover { color: text-primary }` перекрашивает и рамку — она у
    // оригинала `currentColor` строки (ревью ц.18)
    row_group: SharedString,
    border_hover: gpui::Rgba,
    accent: gpui::Rgba,
    tx: &Sender<ShellEvent>,
) -> AnyElement {
    let tx = tx.clone();
    let (view, handle) = (view.to_string(), node.handle.clone());
    let next = if state == CHECKED { UNCHECKED } else { CHECKED };
    let mut b = div()
        .id(SharedString::from(format!("cb:{}:{}", view, node.handle)))
        .w(px(14.0))
        .h(px(14.0))
        .mr(px(4.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.0))
        .border_1()
        // `border: 1px solid var(--border-strong, currentColor)` — токен
        // `--border-strong` в оригинале не определён, работает фоллбек
        // currentColor = цвет строки (ревью ц.7)
        .border_color(border)
        .group_hover(row_group, move |st| st.border_color(border_hover))
        // `role="checkbox" tabIndex=0` + `onKeyDown` пробела/Enter
        // (`TreeViewBody.tsx:121-123,166-170`): чекбокс — таб-стоп и
        // переключается с клавиатуры
        .focus_ring(&format!("ctcb:{view}:{}", node.handle), 3.0, accent)
        .on_key_down({
            let tx = tx.clone();
            let (view, handle) = (view.to_string(), node.handle.clone());
            move |ev: &gpui::KeyDownEvent, _, cx| {
                let k = ev.keystroke.key.as_str();
                if k == "space" || k == "enter" {
                    cx.stop_propagation();
                    let _ = tx.try_send(ShellEvent::Tree(TreeEvent::TreeCheckbox {
                        view: view.clone(),
                        handle: handle.clone(),
                        state: next,
                    }));
                }
            }
        })
        .cursor_pointer()
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            // Тоггл независим от клика по строке
            cx.stop_propagation();
            let _ = tx.try_send(ShellEvent::Tree(TreeEvent::TreeCheckbox {
                view: view.clone(),
                handle: handle.clone(),
                state: next,
            }));
        });
    if state == CHECKED {
        // `.treeCheckbox { font-size: 11 }` сидит на РОДИТЕЛЕ, глиф берёт
        // базовые 16 (`skeleton.css:2-5`) — ревью ц.13
        b = b.child(codicon(CHECK, 16.0));
    }
    if let Some(tip) = node.checkbox_tooltip.clone() {
        b = b.tooltip(crate::ui::tooltip::tooltip(tip));
    }
    b.into_any_element()
}
