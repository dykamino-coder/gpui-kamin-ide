//! Contributed-вью расширений внутри панели: хедер вью, секция дерева
//! (`TreeDataProvider`) и секция вебвью (`WebviewView`).
//!
//! Вынесено из `root.rs` без изменения поведения.

use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::rgba;
use crate::host_link;
use crate::root::RootView;

impl RootView {
    pub(crate) fn contrib_view_header(
        &self,
        d: &crate::activity::DynView,
        p: &'static Palette,
    ) -> AnyElement {
        let meta = self.trees.get(&d.id).map(|t| &t.meta);
        let title = meta
            .and_then(|m| m.title.clone())
            .unwrap_or_else(|| d.name.clone());
        let mut row = div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .px(px(m::SPACE_3))
            .py(px(m::SPACE_1))
            .text_size(px(m::FS_XS))
            .letter_spacing(px(m::FS_XS * 0.04))
            .text_color(rgba(p.text_muted))
            .child(gpui::SharedString::from(title.to_uppercase()));
        if let Some(desc) = meta.and_then(|m| m.description.clone()) {
            row = row.child(
                div()
                    .ml(px(m::SPACE_2))
                    .font_weight(gpui::FontWeight::NORMAL)
                    .opacity(0.55)
                    .child(gpui::SharedString::from(desc.to_uppercase())),
            );
        }
        if let Some((value, tip)) = meta.and_then(|m| m.badge.clone()) {
            let mut badge = div()
                .id(gpui::SharedString::from(format!("badge:{}", d.id)))
                .ml_auto()
                .min_w(px(18.0))
                .px(px(5.0))
                .rounded(px(9.0))
                .bg(rgba(p.accent_primary))
                .text_color(rgba(p.bg_base))
                .text_size(px(m::FS_XS * 0.75))
                .line_height(px(16.0))
                .text_center()
                .child(gpui::SharedString::from(value.to_uppercase()));
            if let Some(t) = tip {
                badge = badge.tooltip(crate::ui::tooltip::tooltip(t));
            }
            row = row.child(badge);
        }
        row.into_any_element()
    }

    /// Секция contributed tree-вью: хедер + тело TreeViewBody.
    /// Корневой уровень и мета запрашиваются при первом показе.
    pub(crate) fn contributed_tree_section(
        &mut self,
        d: &crate::activity::DynView,
        p: &'static Palette,
    ) -> AnyElement {
        {
            let st = self.trees.entry(d.id.clone()).or_default();
            if !st.root_requested {
                st.root_requested = true;
                host_link::request_tree_meta(self.tx.clone(), d.id.clone());
                host_link::request_tree_children(self.tx.clone(), d.id.clone(), String::new());
                // `treeDnd` мог прилететь бродкастом ДО подписки — тянем сами
                host_link::request_tree_dnd(self.tx.clone(), d.id.clone());
            }
        }
        // Узел, пришедший УЖЕ раскрытым (collapsibleState == Expanded), в
        // оригинале монтирует свой TreeLevel и тянет детей сам; у нас запрос
        // идёт по клику, поэтому такие уровни добираем здесь (ревью ц.7).
        if let Some(st) = self.trees.get(&d.id) {
            let mut want: Vec<String> = Vec::new();
            for nodes in st.levels.values().flatten() {
                for n in nodes {
                    if n.collapsible != 0
                        && st
                            .expanded
                            .get(&n.handle)
                            .copied()
                            .unwrap_or(n.collapsible == 2)
                        && !st.levels.contains_key(&n.handle)
                    {
                        want.push(n.handle.clone());
                    }
                }
            }
            if !want.is_empty() {
                let st = self.trees.entry(d.id.clone()).or_default();
                for handle in want {
                    st.levels.insert(handle.clone(), None);
                    host_link::request_tree_children(self.tx.clone(), d.id.clone(), handle);
                }
            }
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h(px(0.))
            .child(self.contrib_view_header(d, p))
            .child(crate::ui::contributed_tree::tree_view_body(
                &d.id,
                self.trees.get(&d.id),
                &self.tx,
                p,
            ))
            .into_any_element()
    }
}
