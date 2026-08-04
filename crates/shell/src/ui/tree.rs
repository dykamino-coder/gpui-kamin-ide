//! Generic Tree (`components/tree/Tree.tsx` + `Tree.module.css`) — элементы
//! 102/103. Это НЕ файловое дерево: у него свой рецепт (индент 14, padding
//! 4/8, gap 8, базовый цвет text-primary, бокс шеврона 14, иконка папки
//! accent-yellow, правая колонка `meta` моно fs-xs).
//!
//! Раньше семпл Design-панели рисовал вместо него копию рецепта file-tree,
//! из-за чего расходилось каждое свойство (ревью ц.13).
//!
//! Модель управляемая: `expanded` (набор раскрытых id) и `selected` держит
//! вызывающий, узел только сообщает о клике.

use std::collections::HashSet;

use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, linear_color_stop, linear_gradient, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::{rgba, tint};

/// `INDENT_PX = 14` — свой шаг, не 12 + 8 файлового дерева.
const INDENT_PX: f32 = 14.0;

#[derive(Clone)]
pub struct TreeNode {
    pub id: SharedString,
    pub label: SharedString,
    /// `"dir"` — глиф папки и возможные дети; `"file"` — лист.
    pub dir: bool,
    /// Правая вторичная подпись (размер, счётчик, статус).
    pub meta: Option<SharedString>,
    /// Переопределение кодикона; иначе `folder`/`file`.
    pub icon: Option<&'static str>,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn file(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string().into(),
            label: label.to_string().into(),
            dir: false,
            meta: None,
            icon: None,
            children: Vec::new(),
        }
    }

    pub fn dir(id: &str, label: &str, children: Vec<TreeNode>) -> Self {
        Self {
            id: id.to_string().into(),
            label: label.to_string().into(),
            dir: true,
            meta: None,
            icon: None,
            children,
        }
    }

    pub fn with_meta(mut self, meta: &str) -> Self {
        self.meta = Some(meta.to_string().into());
        self
    }
}

/// Дерево целиком. `on_click(id, is_dir)` — тоггл раскрытия у папки плюс выбор
/// (оригинал делает и то, и другое одним обработчиком).
pub fn tree(
    nodes: &[TreeNode],
    expanded: &HashSet<String>,
    selected: Option<&str>,
    p: &Palette,
    on_click: impl Fn(&str, bool) + Clone + 'static,
) -> AnyElement {
    let mut out: Vec<AnyElement> = Vec::new();
    for n in nodes {
        push_row(&mut out, n, 0, expanded, selected, p, &on_click);
    }
    div().flex().flex_col().children(out).into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn push_row(
    out: &mut Vec<AnyElement>,
    node: &TreeNode,
    depth: usize,
    expanded: &HashSet<String>,
    selected: Option<&str>,
    p: &Palette,
    on_click: &(impl Fn(&str, bool) + Clone + 'static),
) {
    let is_open = node.dir && expanded.contains(node.id.as_ref());
    let is_selected = selected == Some(node.id.as_ref());
    let has_children = node.dir && !node.children.is_empty();
    let glyph = node.icon.unwrap_or(if node.dir {
        "\u{ea83}" // codicon-folder
    } else {
        "\u{ea7b}" // codicon-file
    });
    let hover_bg = tint(rgba(p.bg_surface), 0.55);
    let cb = on_click.clone();
    let id_owned = node.id.to_string();
    let is_dir = node.dir;

    let mut row = div()
        .id(SharedString::from(format!("tree-{}", node.id)))
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .w_full()
        // `padding: 4px var(--space-2)`, а инлайновый `paddingLeft` перебивает
        // левую часть шортхенда: на глубине 0 слева НОЛЬ
        .py(px(4.0))
        .pr(px(m::SPACE_2))
        .pl(px(depth as f32 * INDENT_PX))
        // Прозрачная рамка зарезервирована под акцентную у выделенной строки
        .border_1()
        .border_color(gpui::transparent_black())
        .rounded(px(m::RADIUS_XS))
        .text_size(px(m::FS_SM))
        .text_color(rgba(p.text_primary))
        .cursor_pointer()
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            cb(&id_owned, is_dir);
        })
        .child(
            // `.chevron { width: 14; place-items: center; font-size: 10 }`,
            // но глиф внутри — отдельный `<i class="codicon">`, и базовые 16
            // модулем не перебиты; `.chevronHidden` прячет, сохраняя место
            div()
                .w(px(14.0))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .text_color(rgba(p.text_muted))
                .child(if has_children {
                    crate::ui::icon::codicon(
                        if is_open {
                            crate::ui::icon::CHEVRON_DOWN
                        } else {
                            crate::ui::icon::CHEVRON_RIGHT
                        },
                        16.0,
                    )
                } else {
                    div()
                }),
        )
        .child(
            // `.iconDir/.iconFile { font-size: var(--fs-sm) }` стоят на ТОМ ЖЕ
            // элементе, что и `.codicon`, то есть (0,1,0) — и ПРОИГРЫВАЮТ
            // вендорной базе `.codicon[class*=codicon-]` (0,2,0). Значит в
            // оригинале глиф 16, а не 12 (ревью ц.13; прежний комментарий
            // формулировал правило наоборот)
            crate::ui::icon::codicon_str(glyph, 16.0)
                .flex_shrink_0()
                .text_color(rgba(if node.dir {
                    p.accent_yellow
                } else {
                    p.text_muted
                })),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(node.label.clone()),
        );

    if let Some(meta) = &node.meta {
        row = row.child(
            div()
                .flex_shrink_0()
                .font_family(crate::ui::design_panel::MONO)
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                .child(meta.clone()),
        );
    }

    if is_selected {
        // `.row.selected` — градиент accent 26 % → 14 %, рамка 45 %
        row = row
            .bg(linear_gradient(
                90.,
                linear_color_stop(tint(rgba(p.accent_primary), 0.26), 0.),
                linear_color_stop(tint(rgba(p.accent_primary), 0.14), 1.),
            ))
            .border_color(tint(rgba(p.accent_primary), 0.45));
    } else {
        row = row.hover(move |s| s.bg(hover_bg));
    }
    out.push(row.into_any_element());

    if is_open {
        for c in &node.children {
            push_row(out, c, depth + 1, expanded, selected, p, on_click);
        }
    }
}
