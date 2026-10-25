//! Итеративная материализация: родитель и курсор детей хранятся в куче.

use super::{Inputs, Materialized};
use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::style::computed::Computed;
use std::rc::Rc;

/// Проход `materialize`. Родитель-элемент — не обязательно узел над элементом в дереве
/// коробок: анонимная обёртка (`Marks::anonymous_wrapper`) пропускает родителя насквозь,
/// перенесённая коробка (`Marks::style_parent`) берёт стиль исходного родителя, который
/// проход уже встретил раньше по порядку документа (`lent`).
pub(super) fn walk(
    nodes: &[Node],
    parent: &Rc<Computed>,
    generation: Inputs,
    lent: &mut Vec<(u64, Rc<Computed>)>,
    vcx: ValueCx<'_>,
) {
    let mut pending = vec![(nodes.iter(), parent.clone())];
    while let Some((nodes, inherited)) = pending.last_mut() {
        let Some(node) = nodes.next() else {
            pending.pop();
            continue;
        };
        let Node::Element(e) = node else { continue };
        let moved = e.marks.style_parent;
        // Вне прохода по документу (`rematerialize`) родитель дан уже исходный.
        let source = moved.and_then(|id| lent.iter().rev().find(|(k, _)| *k == id));
        let parent = source
            .map(|(_, c)| c.clone())
            .unwrap_or_else(|| inherited.clone());
        let slot = e.style.slot();
        let computed = slot
            .get(e.style.cascaded(), &parent, generation)
            .unwrap_or_else(|| slot.compute(&parent, e.style.cascaded(), vcx));
        // Равные входы позволяют разделить значение, но ссылка наследования
        // должна указывать на текущего родителя, включая новый кадр каскада.
        if !e
            .style
            .published()
            .is_some_and(|old| old.ptr_eq(&computed.handle))
            || !slot
                .0
                .borrow()
                .as_ref()
                .is_some_and(|m| Rc::ptr_eq(&m.parent, &parent))
        {
            *slot.0.borrow_mut() = Some(Rc::new(Materialized {
                parent: parent.clone(),
                generation,
                computed: computed.clone(),
            }));
        }
        if e.marks.lends_style {
            lent.push((e.node_id, computed.legacy.clone()));
        }
        let inside = if e.marks.anonymous_wrapper {
            parent
        } else {
            computed.legacy
        };
        pending.push((e.children.iter(), inside));
    }
}
