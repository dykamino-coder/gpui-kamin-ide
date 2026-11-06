//! Равенство стилей коробок как входа перевода в стиль элемента: все значения, кроме ключей
//! коробки в реестрах кадра.

use super::*;

impl BoxStyle {
    /// Equal box values apart from the box's own registry keys (`self_node`, `cb_node`,
    /// `fixed_cb_node`, `anchor_seq`, `anchor_key`): two boxes for which this holds style an
    /// element identically. As Stylo's sharing cache and Blink's matched properties cache do,
    /// it compares inputs, not element identity: the shared computed style by allocation first,
    /// then by value. The destructuring is exhaustive, so a new field has to be classified here.
    pub fn same_box_values(&self, other: &Self) -> bool {
        let Self {
            paint,
            context,
            canvas_background,
            background_paint,
            inline_format,
            inline_decoration,
            style,
            element_style,
            try_style,
            self_node: _,
            cb_node: _,
            fixed_cb_node: _,
            cb_scroll_insets,
            anchor_seq: _,
            anchor_key: _,
            stretched,
            cb_ancestor,
            cb_depth,
            sc_ancestor,
            inline_definite,
            cb_height_def,
            quirk_pct_base,
        } = self;
        *stretched == other.stretched
            && *cb_ancestor == other.cb_ancestor
            && *cb_depth == other.cb_depth
            && *sc_ancestor == other.sc_ancestor
            && *cb_height_def == other.cb_height_def
            && *inline_definite == other.inline_definite
            && *quirk_pct_base == other.quirk_pct_base
            && *cb_scroll_insets == other.cb_scroll_insets
            && *background_paint == other.background_paint
            && *inline_format == other.inline_format
            && *inline_decoration == other.inline_decoration
            && *context == other.context
            && *canvas_background == other.canvas_background
            && same_rc(paint, &other.paint)
            && element_style.as_ref().map(identity) == other.element_style.as_ref().map(identity)
            && try_style.is_none()
            && other.try_style.is_none()
            && (Rc::ptr_eq(style, &other.style) || **style == *other.style)
    }
}

fn same_rc<T: PartialEq>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b) || **a == **b,
        (a, b) => a.is_none() && b.is_none(),
    }
}

/// The element-stage styles of a box by allocation: the variant chain is identity data.
fn identity(e: &ElementStyle) -> (*const Computed, *const Computed, Option<*const ElementStyle>) {
    (
        Rc::as_ptr(&e.original),
        Rc::as_ptr(&e.computed),
        e.parent.as_ref().map(Rc::as_ptr),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_keys_do_not_separate_equal_boxes() {
        let shared = Rc::new(Computed::default());
        let a = BoxStyle::shared(shared.clone());
        let mut b = BoxStyle::shared(shared);
        (b.self_node, b.cb_node, b.fixed_cb_node, b.anchor_seq, b.anchor_key) = (7, 3, 2, 9, 11);
        assert!(a.same_box_values(&b));
    }

    #[test]
    fn values_separate_boxes_even_with_one_allocation() {
        let shared = Rc::new(Computed::default());
        let a = BoxStyle::shared(shared.clone());
        let mut b = BoxStyle::shared(shared.clone());
        b.stretched = true;
        assert!(!a.same_box_values(&b));
        let mut c = BoxStyle::shared(shared);
        c.context.parent_flex_grid = true;
        assert!(!a.same_box_values(&c));
    }

    #[test]
    fn equal_styles_in_two_allocations_match() {
        let a = BoxStyle::new(Computed::default());
        let b = BoxStyle::new(Computed::default());
        assert!(a.same_box_values(&b));
        let mut other = Computed::default();
        other.block_tag = true;
        assert!(!a.same_box_values(&BoxStyle::new(other)));
    }
}
