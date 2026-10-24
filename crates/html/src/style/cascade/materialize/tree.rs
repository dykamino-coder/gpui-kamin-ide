//! Материализация по исходным родителям элементов, включая анонимные обёртки.
use super::*;

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
    for node in nodes {
        let Node::Element(e) = node else { continue };
        let moved = e.marks.style_parent;
        // Вне прохода по документу (`rematerialize`) родитель дан уже исходный.
        let source = moved.and_then(|id| lent.iter().rev().find(|(k, _)| *k == id));
        let parent = source
            .map(|(_, c)| c.clone())
            .unwrap_or_else(|| parent.clone());
        let slot = e.style.slot();
        let computed = slot
            .get(e.style.cascaded(), &parent, generation)
            .unwrap_or_else(|| slot.compute(&parent, e.style.cascaded(), vcx));
        // Равные входы позволяют разделить значение, но ссылка наследования
        // должна указывать на текущего родителя, включая новый кадр каскада.
        if !slot
            .with_parent()
            .is_some_and(|(old, p)| Rc::ptr_eq(&old, &computed) && Rc::ptr_eq(&p, &parent))
        {
            *slot.0.borrow_mut() = Some(Rc::new(Materialized {
                parent: parent.clone(),
                generation,
                computed: computed.clone(),
            }));
        }
        if e.marks.lends_style {
            lent.push((e.node_id, computed.clone()));
        }
        let inside = if e.marks.anonymous_wrapper {
            &parent
        } else {
            &computed
        };
        walk(&e.children, inside, generation, lent, vcx);
    }
}
