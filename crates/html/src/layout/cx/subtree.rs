//! Факты поддерева: один итеративный post-order проход каждого COW-снимка.

use crate::dom::{Element, Node};
use crate::layout::box_kind::{block_level_in_flow, out_of_flow, real_inline};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use std::collections::HashMap;

/// Отрицательный ответ разрешает пропустить семантический обход. Неизвестные
/// длины и сложные inline-коробки остаются на общем пути схлопывания.
#[derive(Default)]
pub(crate) struct SubtreeFacts {
    children: HashMap<usize, (std::rc::Weak<Vec<Node>>, u8)>,
    base: Option<std::rc::Rc<SubtreeFacts>>,
}

const BREAK: u8 = 1;
const FLOAT: u8 = 2;
const MARGIN: u8 = 4;
const BLOCK: u8 = 8;
const CLEARABLE: u8 = 16;
const SPLIT: u8 = 32;

fn own(e: &Element, descendants: u8) -> u8 {
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
    let split = inline && !out_of_flow(&style) && descendants & BLOCK != 0;
    let zero = |v| matches!(v, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    let margins = floats
        || split
        || !zero(style.reset_box_layout.margin.top)
        || !zero(style.reset_box_layout.margin.bottom);
    let block = if inline || style.reset_box_layout.display == Some(Display::Contents) {
        !out_of_flow(&style) && descendants & BLOCK != 0
    } else {
        block_level_in_flow(e)
    };
    // BLOCK — видимость через inline/contents, остальные биты — объединение
    // всех потомков. Граница блочной коробки прерывает только поиск BLOCK.
    (descendants & !BLOCK)
        | u8::from(breaks)
        | (u8::from(floats) * FLOAT)
        | (u8::from(margins) * MARGIN)
        | (u8::from(block) * BLOCK)
        | (u8::from(may_clear) * CLEARABLE)
        | (u8::from(split) * SPLIT)
}

mod preparation;
pub(crate) use preparation::PreparationCache;

impl SubtreeFacts {
    pub(crate) fn overlay(base: std::rc::Rc<Self>) -> Self {
        Self {
            base: Some(base),
            ..Self::default()
        }
    }

    fn cached(&self, identity: usize) -> Option<u8> {
        self.children
            .get(&identity)
            .map(|entry| entry.1)
            .or_else(|| self.base.as_ref().and_then(|base| base.cached(identity)))
    }

    pub(crate) fn breaks_absent(&mut self, root: &Element) -> bool {
        self.element_flags(root) & BREAK == 0
    }

    pub(crate) fn floats_absent(&mut self, root: &Element) -> bool {
        self.element_flags(root) & FLOAT == 0
    }

    pub(crate) fn margins_unchanged(&mut self, nodes: &[Node]) -> bool {
        self.collect(nodes);
        self.list_flags(nodes) & MARGIN == 0
    }

    pub(crate) fn inline_preparation_absent(&mut self, nodes: &[Node]) -> bool {
        self.collect(nodes);
        self.list_flags(nodes) & SPLIT == 0
    }

    pub(crate) fn clearance_possible(&mut self, nodes: &[Node]) -> bool {
        self.collect(nodes);
        self.list_flags(nodes) & CLEARABLE != 0
    }

    fn element_flags(&mut self, root: &Element) -> u8 {
        self.collect(&root.children);
        own(root, self.list_flags(&root.children))
    }

    fn list_flags(&self, nodes: &[Node]) -> u8 {
        nodes.iter().fold(0, |flags, node| {
            flags
                | match node {
                    Node::Element(e) => own(
                        e,
                        self.cached(e.children.identity())
                            .expect("post-order facts"),
                    ),
                    Node::Text(_) => 0,
                }
        })
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
            if self.cached(children.identity()).is_some() {
                continue;
            }
            if leave {
                let flags = self.list_flags(children);
                // Метка снимка исключает повторное использование адреса, не
                // удерживая промежуточные деревья сборки до конца кадра.
                self.children
                    .insert(children.identity(), (children.mark(), flags));
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
