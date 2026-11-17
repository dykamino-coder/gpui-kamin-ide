//! Факты поддерева: один итеративный post-order проход каждого COW-снимка.

use crate::dom::{BoxTextFacts, BoxTreeSummary, Element, Node};
use crate::layout::box_kind::{block_level_in_flow, out_of_flow, real_inline};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Отрицательный ответ разрешает пропустить семантический обход. Неизвестные
/// длины и сложные inline-коробки остаются на общем пути схлопывания.
#[derive(Clone, Copy, Default)]
pub(crate) struct SubtreeFacts {}

fn own(e: &Element, descendants: BoxTreeSummary) -> BoxTreeSummary {
    crate::work::add!(SubtreeNodesVisited, 1);
    let style = e.style.used();
    let breaks = style.reset_fragmentation.break_before_force
        || style.reset_fragmentation.break_after_force
        || style.reset_fragmentation.break_before_avoid
        || style.reset_fragmentation.break_after_avoid;
    let floats = style.reset_box_layout.float.is_some_and(|f| f != 0);
    let clearable = |c: &Computed| {
        c.reset_box_layout.float.is_some_and(|f| f != 0)
            || c.reset_text_box.initial_letter.is_some()
    };
    let may_clear = clearable(&style)
        || e.first_letter
            .as_ref()
            .is_some_and(|s| clearable(s.cascaded()))
        || e.hover.as_ref().is_some_and(|s| clearable(s.cascaded()));
    let inline = real_inline(e);
    // CSS2 §9.2.1.1: простой inline с текстом не требует переписывать поля,
    // но блок в его потомках требует разбиения даже при нулевых полях.
    let split = inline && !out_of_flow(&style) && descendants.block;
    let zero = |v| matches!(v, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    let margins = floats
        || split
        || !zero(style.reset_box_layout.margin.top)
        || !zero(style.reset_box_layout.margin.bottom);
    let block = if inline || style.reset_box_layout.display == Some(Display::Contents) {
        !out_of_flow(&style) && descendants.block
    } else {
        block_level_in_flow(e)
    };
    // BLOCK — видимость через inline/contents, остальные биты — объединение
    // всех потомков. Граница блочной коробки прерывает только поиск BLOCK.
    BoxTreeSummary {
        breaks: descendants.breaks || breaks,
        floats: descendants.floats || floats,
        margins: descendants.margins || margins,
        block,
        clearable: descendants.clearable || may_clear,
        split_inline: descendants.split_inline || split,
        text: BoxTextFacts {
            elements: true,
            in_flow: descendants.text.in_flow
                && style.reset_box_layout.display != Some(Display::None)
                && !out_of_flow(&style)
                && !floats,
            ..Default::default()
        },
    }
}

mod preparation;
#[cfg(test)]
pub(crate) use preparation::Preparation;
pub(crate) use preparation::PreparationCache;

impl SubtreeFacts {
    pub(crate) fn breaks_absent(&mut self, root: &Element) -> bool {
        !self.element_flags(root).breaks
    }

    pub(crate) fn floats_absent(&mut self, root: &Element) -> bool {
        !self.element_flags(root).floats
    }

    pub(crate) fn margins_unchanged(&mut self, nodes: &[Node]) -> bool {
        self.collect(nodes);
        !self.list_flags(nodes).margins
    }

    pub(crate) fn inline_preparation_absent(&mut self, nodes: &[Node]) -> bool {
        self.collect(nodes);
        !self.list_flags(nodes).split_inline
    }

    pub(crate) fn clearance_possible(&mut self, nodes: &[Node]) -> bool {
        self.collect(nodes);
        self.list_flags(nodes).clearable
    }

    fn element_flags(&mut self, root: &Element) -> BoxTreeSummary {
        let descendants = root.children.box_summary().unwrap_or_else(|| {
            self.collect(&root.children);
            let summary = self.list_flags(&root.children);
            root.children.publish_box_summary(summary);
            summary
        });
        own(root, descendants)
    }

    /// Только непосредственные дети: подготовленные потомки не обходятся.
    /// Новые layout-made списки явно переходят на обычный запрос ниже.
    pub(crate) fn prepared_list(&self, nodes: &[Node]) -> Option<BoxTreeSummary> {
        nodes
            .iter()
            .try_fold(BoxTreeSummary::default(), |flags, node| {
                Some(
                    flags
                        | match node {
                            Node::Element(e) => own(e, e.children.box_summary()?),
                            Node::Text(text) => BoxTreeSummary {
                                text: BoxTextFacts::text(text),
                                ..Default::default()
                            },
                        },
                )
            })
    }

    fn list_flags(&self, nodes: &[Node]) -> BoxTreeSummary {
        self.prepared_list(nodes).expect("post-order facts")
    }

    fn collect(&mut self, nodes: &[Node]) {
        crate::work::add!(SubtreeQueries, 1);
        let mut pending = Vec::new();
        for node in nodes {
            if let Node::Element(e) = node {
                pending.push((&e.children, false));
            }
        }
        while let Some((children, leave)) = pending.pop() {
            if children.box_summary().is_some() {
                continue;
            }
            if leave {
                let flags = self.list_flags(children);
                children.publish_box_summary(flags);
            } else {
                pending.push((children, true));
                for node in children.iter() {
                    if let Node::Element(e) = node {
                        pending.push((&e.children, false));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
