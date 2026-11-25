//! The damage level of a change of computed style: what the frame must redo
//! for an element whose style goes from one value to another (Blink
//! `StyleDifference` from `ComputedStyle::VisualInvalidationDiff` and
//! `LayoutObject::SetStyle`, Gecko `nsChangeHint`, Stylo `RestyleDamage`).
//!
//! The levels are ordered: a box tree change (`Rebox`) also needs layout and
//! paint; a layout change (`Layout`) also needs paint. `Paint` is
//! `Computed::same_layout`. `Layout` is a change only of the values layout
//! reads from boxes it keeps: sizes, spacing, borders, flex and grid
//! placement within the same item order, text box trimming, fonts and text
//! flow. Every other value is read by the passes that prepare and build the
//! boxes (display, overflow and scroll containers, containment, positioning,
//! generated content, writing modes, effects and transforms that make
//! layers): its change rebuilds the element's boxes. The state of the
//! cascade itself (the place of the declaration being applied) is no
//! computed value and no damage. The level is a property of the edit
//! record, decided once per element; its executors read only the level.

use super::super::{Computed, ResetBoxLayout, ResetCascade, ResetFlex};

/// What a change of an element's computed style damages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Damage {
    /// No value changed.
    None,
    /// Only values paint reads changed: the boxes repaint in place.
    Paint,
    /// Values layout reads changed; the box tree is the same.
    Layout,
    /// The boxes the element generates change.
    Rebox,
}

impl Computed {
    /// The damage of replacing this style by `new`, with the name of the
    /// first group that decides it (for the `why` journal). `borders` lets
    /// the colours of the borders be paint values
    /// (`same_layout_but_border_colors`).
    pub fn damage(&self, new: &Self, borders: bool) -> (Damage, &'static str) {
        if self == new {
            return (Damage::None, "");
        }
        if let Some(group) = rebox_group(self, new) {
            return (Damage::Rebox, group);
        }
        // Equal but for the state of the cascade.
        let mut same = self.clone();
        same.reset_cascade = new.reset_cascade.clone();
        if same == *new {
            return (Damage::None, "");
        }
        let paint = if borders {
            self.same_layout_but_border_colors(new)
        } else {
            self.same_layout(new)
        };
        match paint {
            true => (Damage::Paint, "paint"),
            false => (Damage::Layout, layout_group(self, new)),
        }
    }
}

/// The cascade metadata of two styles is equal but for the state of the
/// declaration being applied (`decl_order`, `css_wide_value`,
/// `declaration_parse`), which no pass after the cascade reads.
pub(in crate::style::computed) fn same_cascade(a: &ResetCascade, b: &ResetCascade) -> bool {
    if a == b {
        return true;
    }
    let mut a = a.clone();
    a.decl_order = b.decl_order;
    a.css_wide_value = b.css_wide_value;
    a.declaration_parse = b.declaration_parse;
    a == *b
}

/// `same_cascade` but for the cascade order of the sides, sizes and borders
/// (`SideOrder`), which resolves logical against physical declarations of
/// those layout and paint values: a layout change at most.
fn same_cascade_but_sides(a: &ResetCascade, b: &ResetCascade) -> bool {
    if a.side_order == b.side_order {
        return same_cascade(a, b);
    }
    let mut a = a.clone();
    a.side_order = b.side_order.clone();
    same_cascade(&a, b)
}

/// The first value outside the layout and paint values that differs.
fn rebox_group(a: &Computed, b: &Computed) -> Option<&'static str> {
    let (ai, bi) = (&a.inh.0, &b.inh.0);
    let (asvg, bsvg) = (&a.reset_svg_math, &b.reset_svg_math);
    [
        (a.decor_box != b.decor_box, "decor_box"),
        (a.block_tag != b.block_tag, "block_tag"),
        (a.plain_block_box != b.plain_block_box, "plain_block_box"),
        (
            !same_cascade_but_sides(&a.reset_cascade, &b.reset_cascade),
            "cascade",
        ),
        (
            !same_boxes(&a.reset_box_layout, &b.reset_box_layout),
            "box_layout.boxes",
        ),
        (a.reset_flex.order != b.reset_flex.order, "flex.order"),
        (a.reset_context != b.reset_context, "context"),
        (a.reset_generated != b.reset_generated, "generated"),
        (
            asvg != bsvg && !super::svg::same_layout_svg(asvg, bsvg),
            "svg_math",
        ),
        (a.reset_positioned != b.reset_positioned, "positioned"),
        (a.reset_effects != b.reset_effects, "effects"),
        (a.reset_mask != b.reset_mask, "mask"),
        (a.reset_shapes != b.reset_shapes, "shapes"),
        (
            a.reset_fragmentation != b.reset_fragmentation,
            "fragmentation",
        ),
        (a.reset_transforms != b.reset_transforms, "transforms"),
        (ai.table_list != bi.table_list, "inh.table_list"),
        (ai.text_ink != bi.text_ink, "inh.text_ink"),
        (ai.writing != bi.writing, "inh.writing"),
    ]
    .into_iter()
    .find_map(|(differs, name)| differs.then_some(name))
}

/// The values of the box layout group that decide which boxes and scroll
/// containers the element generates are equal.
fn same_boxes(a: &ResetBoxLayout, b: &ResetBoxLayout) -> bool {
    a.display == b.display
        && a.overflow_x == b.overflow_x
        && a.overflow_y == b.overflow_y
        && a.scroll == b.scroll
        && a.container_type == b.container_type
        && a.custom_scrollbar == b.custom_scrollbar
        && a.resize == b.resize
        && a.snap == b.snap
}

/// The first layout group that differs.
fn layout_group(a: &Computed, b: &Computed) -> &'static str {
    let (ai, bi) = (&a.inh.0, &b.inh.0);
    let flex = |f: &ResetFlex| ResetFlex {
        order: None,
        ..f.clone()
    };
    [
        (
            a.reset_cascade.side_order != b.reset_cascade.side_order,
            "cascade.sides",
        ),
        (a.reset_text_box != b.reset_text_box, "text_box"),
        (a.reset_box_layout != b.reset_box_layout, "box_layout"),
        (flex(&a.reset_flex) != flex(&b.reset_flex), "flex"),
        (a.reset_border != b.reset_border, "border"),
        (a.reset_grid != b.reset_grid, "grid"),
        (a.reset_background != b.reset_background, "background"),
        (ai.font != bi.font, "inh.font"),
        (ai.text_flow != bi.text_flow, "inh.text_flow"),
        (ai.ui != bi.ui, "inh.ui"),
    ]
    .into_iter()
    .find_map(|(differs, name)| differs.then_some(name))
    .unwrap_or("other")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Display;
    use crate::style::values::value::{Color, Len};

    fn grey() -> Option<Color> {
        Some(Color {
            r: 0.9,
            g: 0.9,
            b: 0.9,
            a: 1.0,
        })
    }

    #[test]
    fn each_change_takes_the_level_of_what_reads_it() {
        let a = Computed::default();
        let level = |f: &dyn Fn(&mut Computed)| {
            let mut b = a.clone();
            f(&mut b);
            a.damage(&b, false)
        };
        assert_eq!(level(&|_| {}), (Damage::None, ""));
        assert_eq!(
            level(&|b| b.reset_background.background = grey()),
            (Damage::Paint, "paint")
        );
        assert_eq!(
            level(&|b| b.reset_box_layout.width = Some(Len::Px(5.0))),
            (Damage::Layout, "box_layout")
        );
        assert_eq!(
            level(&|b| b.reset_box_layout.display = Some(Display::Flex)),
            (Damage::Rebox, "box_layout.boxes")
        );
        assert_eq!(
            level(&|b| b.reset_flex.order = Some(1)),
            (Damage::Rebox, "flex.order")
        );
        assert_eq!(
            level(&|b| b.reset_flex.flex_grow = Some(1.0)),
            (Damage::Layout, "flex")
        );
    }

    #[test]
    fn the_state_of_the_cascade_is_no_damage() {
        let a = Computed::default();
        let mut b = a.clone();
        b.reset_cascade.css_wide_value = true;
        assert_eq!(a.damage(&b, false), (Damage::None, ""));
        b.reset_background.background = grey();
        assert_eq!(a.damage(&b, false), (Damage::Paint, "paint"));
        b.reset_cascade.dark_color_scheme = true;
        assert_eq!(a.damage(&b, false), (Damage::Rebox, "cascade"));
    }

    #[test]
    fn a_layout_change_writes_only_its_groups_into_a_box_that_holds_the_old_ones() {
        let old = Computed::default();
        let mut new = old.clone();
        new.reset_box_layout.width = Some(Len::Px(5.0));
        new.reset_background.background = grey();
        let mut boxed = old.clone();
        boxed.decor_box = 7;
        let written = boxed
            .with_layout_of(&old, &new)
            .expect("the box holds the old groups");
        assert!(written.decor_box == 7 && written.reset_box_layout == new.reset_box_layout);
        assert!(written.reset_background == new.reset_background);
        // The build adjusted a changed group of the box: no write in place.
        boxed.reset_box_layout.height = Some(Len::Px(1.0));
        assert!(boxed.with_layout_of(&old, &new).is_none());
    }
}
