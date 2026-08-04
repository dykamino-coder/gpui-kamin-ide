//! Семпл дерева в дизайн-панели.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::colors::tint;
use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
use crate::ui::ds::state::DesignState;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Семпл дерева = НАСТОЯЩИЙ generic `Tree` (элементы 102/103), а не копия
/// рецепта file-tree: у него свой индент 14, padding 4/8, gap 8, базовый цвет
/// text-primary, бокс шеврона 14, иконка папки accent-yellow и колонка `meta`
/// (ревью ц.13: прежний семпл расходился по каждому свойству).
/// Данные — `SAMPLE_TREE` оригинала (`component-samples.tsx:14-37`).
pub(crate) fn sample_tree(
    design: &DesignState,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    use crate::ui::tree::TreeNode;
    let nodes = vec![
        TreeNode::dir(
            "src",
            "src",
            vec![
                TreeNode::dir(
                    "src/host",
                    "host",
                    vec![
                        TreeNode::file("src/host/index.ts", "index.ts").with_meta("13 KB"),
                        TreeNode::file("src/host/layout-store.ts", "layout-store.ts")
                            .with_meta("2.5 KB"),
                        TreeNode::file("src/host/json-file-store.ts", "json-file-store.ts")
                            .with_meta("1.8 KB"),
                    ],
                ),
                TreeNode::dir(
                    "src/exthost",
                    "exthost",
                    vec![
                        TreeNode::file("src/exthost/api.ts", "api.ts").with_meta("3.0 KB"),
                        TreeNode::file("src/exthost/loader.ts", "loader.ts").with_meta("8.2 KB"),
                    ],
                ),
            ],
        ),
        {
            let mut n = TreeNode::file("package.json", "package.json").with_meta("1.2 KB");
            n.icon = Some("\u{eb0f}"); // codicon-json
            n
        },
        {
            let mut n = TreeNode::file("README.md", "README.md").with_meta("4.1 KB");
            n.icon = Some("\u{eb1d}"); // codicon-markdown
            n
        },
    ];
    // Состояние живёт в `DesignState` — семпл интерактивный, как оригинал
    let expanded = design.tree_expanded.clone();
    let selected = design.tree_selected.clone();
    let tx_tree = tx.clone();
    // `.treeFrame`: max-w 380, p space-2, рамка bg-surface 60 %, r-sm, bg-base
    div()
        .w_full()
        .max_w(px(380.0))
        .p(px(m::SPACE_2))
        .rounded(px(m::RADIUS_SM))
        .border_1()
        .border_color(tint(rgba(p.bg_surface), 0.6))
        .bg(rgba(p.bg_base))
        .child(crate::ui::tree::tree(
            &nodes,
            &expanded,
            Some(selected.as_str()),
            p,
            move |id, is_dir| {
                let _ = tx_tree.try_send(ShellEvent::Cz(CzEvent::DesignSample(
                    crate::ui::design_samples::DesignAction::TreeClick(id.to_string(), is_dir),
                )));
            },
        ))
        .into_any_element()
}
