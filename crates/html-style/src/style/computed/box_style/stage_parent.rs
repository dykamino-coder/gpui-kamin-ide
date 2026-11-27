//! Равенство коробки-родителя как входа стадии коробки ребёнка (`cascade::materialize`,
//! память стадии на слоте стиля узла): всё, что стадия читает у родителя, кроме ключей
//! реестров кадра. Их стадия только переносит (`cb_node`, `fixed_cb_node` от `self_node`),
//! и попадание переносит их заново.

use super::conversion_input::{same_rc, same_values};
use super::*;

impl BoxStyle {
    /// Whether the child box stage under `self` gives the same values as under `other`, apart
    /// from the registry keys it copies from the parent (`cascade::inherit::reidentify`). The
    /// destructuring is exhaustive, so a new field has to be classified here. Blink and Stylo
    /// reuse a child's style the same way: by comparing its inputs (the parent's style and
    /// the matched declarations), never by element identity.
    pub fn same_stage_parent(&self, other: &Self) -> bool {
        let Self {
            paint,
            context,
            canvas_background,
            background_paint,
            inline_format,
            inline_decoration,
            style,
            element_style,
            try_style: _,
            // Frame registry keys: the stage reads them only to copy them (`reidentify`).
            self_node: _,
            cb_node: _,
            fixed_cb_node: _,
            anchor_seq: _,
            anchor_key: _,
            // Not read by the stage: the basis of the children's inline percentages.
            inline_definite: _,
            decor_box,
            cb_scroll_insets,
            stretched,
            cb_ancestor,
            cb_depth,
            sc_ancestor,
            cb_height_def,
            quirk_pct_base,
            flow_align,
            tree,
        } = self;
        *stretched == other.stretched
            && *cb_height_def == other.cb_height_def
            && *flow_align == other.flow_align
            && *tree == other.tree
            && *cb_ancestor == other.cb_ancestor
            && *cb_depth == other.cb_depth
            && *sc_ancestor == other.sc_ancestor
            && *quirk_pct_base == other.quirk_pct_base
            && *cb_scroll_insets == other.cb_scroll_insets
            && *decor_box == other.decor_box
            && *background_paint == other.background_paint
            && *inline_format == other.inline_format
            && *inline_decoration == other.inline_decoration
            && *context == other.context
            && *canvas_background == other.canvas_background
            && same_element(element_style.as_ref(), other.element_style.as_ref())
            && same_rc(paint, &other.paint)
            && (Rc::ptr_eq(style, &other.style) || same_values(style, &other.style))
    }
}

/// The same element stages: the child reads the parent's variant by identity.
fn same_element(a: Option<&ElementStyle>, b: Option<&ElementStyle>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            Rc::ptr_eq(&a.original, &b.original)
                && Rc::ptr_eq(&a.computed, &b.computed)
                && match (&a.parent, &b.parent) {
                    (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                    (x, y) => x.is_none() && y.is_none(),
                }
        }
        (a, b) => a.is_none() && b.is_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_keys_do_not_change_the_stage_parent() {
        let a = BoxStyle::new(Computed::default());
        let mut b = a.clone();
        b.self_node = 7;
        b.cb_node = 8;
        b.fixed_cb_node = 9;
        b.anchor_seq = 3;
        b.anchor_key = 4;
        b.inline_definite = Some(10.0);
        assert!(a.same_stage_parent(&b));
        b.stretched = true;
        assert!(!a.same_stage_parent(&b));
    }

    #[test]
    fn equal_computed_values_in_another_allocation_are_the_same_parent() {
        let a = BoxStyle::new(Computed::default());
        let b = BoxStyle::new(Computed::default());
        assert!(a.same_stage_parent(&b));
        let mut c = BoxStyle::new(Computed::default());
        c.reset_box_layout.display = Some(Display::Flex);
        assert!(!a.same_stage_parent(&c));
    }
}
