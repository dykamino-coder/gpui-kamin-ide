//! Память дерева документа по владельцам (`heap-report`, docs/memprof.md):
//! узлы, текст и атрибуты — DOM; вычисленный стиль — style.

use super::{Element, Marks, Node};
use crate::style::computed::{Computed, InheritedGroup, InheritedStyle};
use kamin_heap::{HeapSize, Ops};
use markup5ever_rcdom::{Handle, NodeData};

/// Исходное дерево разбора, которое удерживает revision (`TraversalState`):
/// блоки узлов `Rc`, списки детей и атрибутов, аренды допуска. Обход без
/// рекурсии: глубина документа не ограничена стеком. Текст тендрилов не меряется.
pub fn source_tree(root: &Handle, ops: &mut Ops) -> usize {
    let mut total = 0;
    let mut pending = vec![root.clone()];
    while let Some(node) = pending.pop() {
        let own = ops.shallow(std::rc::Rc::as_ptr(&node));
        if own == 0 {
            continue;
        }
        total += own;
        node.heap_blocks(&mut |pointer, bytes| {
            if ops.once(pointer) {
                total += ops.block(pointer, bytes);
            }
        });
        if let Ok(children) = node.children.try_borrow() {
            pending.extend(children.iter().cloned());
        }
        if let NodeData::Element {
            template_contents, ..
        } = &node.data
        {
            if let Ok(Some(contents)) = template_contents.try_borrow().as_deref() {
                pending.push(contents.clone());
            }
        }
    }
    total
}

impl HeapSize for Node {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        match self {
            Node::Text(text) => ops.at("dom/text", |ops| text.heap_size(ops)),
            Node::Element(element) => element.heap_size(ops),
        }
    }
}

impl HeapSize for Element {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        let names = ops.at("dom/attributes", |ops| {
            self.tag.heap_size(ops) + self.attrs.heap_size(ops)
        });
        let style = ops.at("style/computed", |ops| {
            self.style.heap_size(ops)
                + self.hover.heap_size(ops)
                + self.first_letter.heap_size(ops)
                + self.first_line.heap_size(ops)
                + self.anim.heap_size(ops)
        });
        let other = ops.at("dom/other", |ops| self.marks.heap_size(ops));
        names + style + other + self.children.heap_size(ops)
    }
}

impl HeapSize for Marks {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        self.state_id.heap_size(ops)
    }
}

impl HeapSize for Computed {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        ops.at("style/groups", |ops| {
            self.reset_text_box.heap_size(ops)
                + self.reset_cascade.heap_size(ops)
                + self.reset_box_layout.heap_size(ops)
                + self.reset_flex.heap_size(ops)
                + self.reset_border.heap_size(ops)
                + self.reset_grid.heap_size(ops)
                + self.reset_context.heap_size(ops)
                + self.reset_generated.heap_size(ops)
                + self.reset_svg_math.heap_size(ops)
                + self.reset_positioned.heap_size(ops)
                + self.reset_effects.heap_size(ops)
                + self.reset_background.heap_size(ops)
                + self.reset_mask.heap_size(ops)
                + self.reset_shapes.heap_size(ops)
                + self.reset_fragmentation.heap_size(ops)
                + self.reset_transforms.heap_size(ops)
                + self.inh.heap_size(ops)
        })
    }
}

impl HeapSize for InheritedGroup {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        ops.at("style/inherited", |ops| self.0.heap_size(ops))
    }
}

impl HeapSize for InheritedStyle {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        ops.at("style/groups", |ops| {
            self.ui.heap_size(ops)
                + self.font.heap_size(ops)
                + self.text_flow.heap_size(ops)
                + self.table_list.heap_size(ops)
                + self.text_ink.heap_size(ops)
                + self.writing.heap_size(ops)
        })
    }
}
