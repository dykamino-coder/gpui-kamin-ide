//! Равенство стилей коробок как входа перевода в стиль элемента: все значения, кроме
//! идентичности коробки (ключи реестров кадра, ссылка на стадии стиля элемента).

use super::*;

impl BoxStyle {
    /// Equal box values apart from the box's own identity: its registry keys (`self_node`,
    /// `cb_node`, `fixed_cb_node`, `anchor_seq`, `anchor_key`), the link to its element's
    /// style stages (`element_style`, read only by `@position-try` resolution and cascade
    /// publication) and the decorating box of its computed style (`same_values`). Two boxes
    /// for which this holds style an element identically. As Stylo's sharing cache and
    /// Blink's matched properties cache do, it compares inputs, not element identity.
    pub fn same_box_values(&self, other: &Self) -> bool {
        self.same_box_context(other)
            && (self.same_style_allocation(other) || self.same_style_values(other))
    }

    /// Equal values of the box context: every field but the computed style and the
    /// decoration paint. The destructuring is exhaustive, so a new field has to be
    /// classified here.
    fn same_box_context(&self, other: &Self) -> bool {
        let Self {
            paint: _,
            context,
            canvas_background,
            background_paint,
            inline_format,
            inline_decoration,
            style: _,
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
    }

    /// The same computed style and decoration paint allocations: equal without reading them.
    fn same_style_allocation(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.style, &other.style)
            && match (&self.paint, &other.paint) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (a, b) => a.is_none() && b.is_none(),
            }
    }

    /// Equal computed style and decoration paint values.
    fn same_style_values(&self, other: &Self) -> bool {
        (Rc::ptr_eq(&self.style, &other.style) || same_values(&self.style, &other.style))
            && same_rc(&self.paint, &other.paint)
    }
}

/// Equal computed values apart from `decor_box`: the identity of the box whose text
/// decorations the element propagates (css-text-decor-3 §2.1), read only by the cascade
/// that propagates them. Exhaustive, like `same_box_values`.
fn same_values(a: &Computed, b: &Computed) -> bool {
    let Computed {
        decor_box: _,
        block_tag,
        plain_block_box,
        clone_audit: _,
        reset_text_box,
        reset_cascade,
        reset_box_layout,
        reset_flex,
        reset_border,
        reset_grid,
        reset_context,
        reset_generated,
        reset_svg_math,
        reset_positioned,
        reset_effects,
        reset_background,
        reset_mask,
        reset_shapes,
        reset_fragmentation,
        reset_transforms,
        inh,
    } = a;
    // Item placement groups first: siblings of one container (flex items with their own
    // `flex`, grid items with their own area) differ there, and a miss compares every
    // candidate, so it should stop before walking the large box layout group.
    *block_tag == b.block_tag
        && *plain_block_box == b.plain_block_box
        && *reset_flex == b.reset_flex
        && *reset_grid == b.reset_grid
        && *reset_positioned == b.reset_positioned
        && *reset_box_layout == b.reset_box_layout
        && *reset_border == b.reset_border
        && *reset_background == b.reset_background
        && *reset_context == b.reset_context
        && *reset_text_box == b.reset_text_box
        && *reset_cascade == b.reset_cascade
        && *reset_generated == b.reset_generated
        && *reset_svg_math == b.reset_svg_math
        && *reset_effects == b.reset_effects
        && *reset_mask == b.reset_mask
        && *reset_shapes == b.reset_shapes
        && *reset_fragmentation == b.reset_fragmentation
        && *reset_transforms == b.reset_transforms
        && *inh == b.inh
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
        (
            b.self_node,
            b.cb_node,
            b.fixed_cb_node,
            b.anchor_seq,
            b.anchor_key,
        ) = (7, 3, 2, 9, 11);
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
        let b = BoxStyle::new(Computed {
            decor_box: 12,
            ..Default::default()
        });
        assert!(a.same_box_values(&b));
        let mut other = Computed::default();
        other.block_tag = true;
        assert!(!a.same_box_values(&BoxStyle::new(other)));
    }
}
