//! Каскад: вычисление элемента заново под его родителем-элементом — после правки каскадного
//! стиля кадром анимации и с объявлениями источника position-try.

use crate::document::state::ValueCx;
use crate::dom::{Node, SharedStyle};
use crate::style::computed::{BoxStyle, Computed, ElementStyle};
use std::rc::Rc;

/// Материализовать заново элемент `e`, чей каскадный стиль правлен после материализации
/// (кадр анимации — источник каскада), и его поддерево под тем же родителем-элементом, что и
/// у исходного `original`: потомки с каскадным `inherit` видят новые значения.
pub(crate) fn rematerialize(
    original: &SharedStyle,
    e: crate::dom::Element,
    vcx: ValueCx<'_>,
) -> crate::dom::Element {
    let Some((_, parent)) = original.slot().with_parent() else {
        return e;
    };
    let nodes = [Node::Element(e)];
    super::materialize(&nodes, &parent, vcx);
    let [Node::Element(e)] = nodes else {
        unreachable!("one element in, one element out")
    };
    e
}

/// Вычислить элемент заново с дополнительными объявлениями `edit` (источник каскада
/// position-try, css-anchor-position-1 §fallback-rule) под его родителем-элементом и
/// положить изменившиеся поля на стиль коробки `c`: значения проходят ту же стадию
/// вычисления (zoom, шрифтовые единицы, `inherit`), что и остальные. `false` — у стиля
/// нет материализации (стиль одной стадии), объявления не применены.
pub(crate) fn recompute_onto(
    c: &mut BoxStyle,
    own: &SharedStyle,
    edit: impl FnOnce(&mut Computed),
    vcx: ValueCx<'_>,
) -> bool {
    let Some((materialized, parent)) = own.slot().with_parent() else {
        return false;
    };
    let mut declared = own.cascaded().clone();
    edit(&mut declared);
    let context = c.element_style.as_ref().and_then(|p| p.parent.clone());
    let parent = context
        .as_ref()
        .and_then(|p| p.variant_for(&parent))
        .unwrap_or(&parent);
    let computed = Rc::new(crate::style::cascade::compute::compute(
        parent, &declared, vcx,
    ));
    let previous = c
        .element_style
        .as_ref()
        .map(|p| p.computed.clone())
        .unwrap_or_else(|| materialized.clone());
    super::used::lay_used(c, &computed, &previous);
    c.element_style = Some(ElementStyle {
        original: materialized,
        computed,
        parent: context,
    });
    true
}
