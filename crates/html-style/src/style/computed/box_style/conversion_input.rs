//! Равенство стилей коробок как входа перевода в стиль элемента: все значения, кроме
//! идентичности коробки (ключи реестров кадра, ссылка на стадии стиля элемента).

use super::*;

impl BoxStyle {
    /// Equal box values apart from the box's own identity: its registry keys (`self_node`,
    /// `cb_node`, `fixed_cb_node`, `anchor_seq`, `anchor_key`) and the link to its element's
    /// style stages (`element_style`, read only by `@position-try` resolution and cascade
    /// publication). Two boxes for which this holds style an element identically. As Stylo's sharing cache and Blink's matched properties cache do,
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
            element_style: _,
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
            && try_style.is_none()
            && other.try_style.is_none()
            && (Rc::ptr_eq(style, &other.style) || **style == *other.style)
            && same_rc(paint, &other.paint)
    }
}

fn same_rc<T: PartialEq>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b) || **a == **b,
        (a, b) => a.is_none() && b.is_none(),
    }
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
    fn element_stage_links_do_not_separate_boxes() {
        let shared = Rc::new(Computed::default());
        let a = BoxStyle::shared(shared.clone());
        let mut b = BoxStyle::shared(shared.clone());
        b.element_style = Some(ElementStyle {
            original: Rc::new(Computed::default()),
            computed: shared,
            parent: None,
        });
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
