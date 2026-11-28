//! The name of the first parent fact two keys differ in: the journal reason of a box stage
//! miss by its parent (`stage_memo`), so a frame names what of the parents changed.

use super::{LayoutFacts, ParentFacts, TreeFacts};

macro_rules! first_field {
    ($a:expr, $b:expr, $prefix:literal, $($field:ident),+ $(,)?) => {
        $(if $a.$field != $b.$field {
            return Some(concat!($prefix, stringify!($field)));
        })+
    };
}

impl ParentFacts<'_> {
    /// The first fact that differs from `other`, or `None` when they are equal.
    pub fn first_difference(&self, other: &ParentFacts<'_>) -> Option<&'static str> {
        layout(&self.layout, &other.layout).or_else(|| tree(&self.tree, &other.tree))
    }

    /// Both values of the fact `first_difference` named, for the journal.
    pub fn difference_detail(&self, other: &ParentFacts<'_>) -> String {
        let (a, b) = (format!("{self:?}"), format!("{other:?}"));
        let start = (a.char_indices().zip(b.chars()))
            .find(|((_, x), y)| x != y)
            .map_or(a.len().min(b.len()), |((i, _), _)| i);
        let from = a[..start].rfind(", ").map_or(0, |i| i + 2);
        let cut = |s: &str| {
            s.get(from..)
                .unwrap_or("")
                .chars()
                .take(160)
                .collect::<String>()
        };
        format!("{} -> {}", cut(&a), cut(&b))
    }
}

fn layout(a: &LayoutFacts, b: &LayoutFacts) -> Option<&'static str> {
    first_field!(
        a,
        b,
        "layout.",
        context,
        cb_ancestor,
        cb_depth,
        sc_ancestor,
        cb_scroll_insets,
        fragment_clip_ancestor,
        in_multicol,
        backdrop_root_above,
        transform_ancestor,
        transformed_ancestor,
        orthogonal_scrollport,
        flex_or_grid,
        laid_out,
        rel_shift,
        box_shift,
        child_cb_height_def,
        quirk_pct_base,
        height,
        ortho_limit,
        orthogonal_inline,
        line_clamp,
        reversed_axes,
    );
    None
}

fn tree(a: &TreeFacts<'_>, b: &TreeFacts<'_>) -> Option<&'static str> {
    first_field!(
        a,
        b,
        "tree.",
        first_letter_excluded,
        text_shadow_ink,
        ruby_unit,
        ruby_suppress_breaks,
        turned_line,
        lines_reversed,
        inline_format,
        flat_mode,
        inline_decoration,
        no_select,
        foreground,
        filter,
        filter_ops,
        font_px,
        blockified,
        decors,
    );
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_fact_is_named_and_equal_facts_name_none() {
        let parsed = crate::dom::parse("<p>a</p>", "");
        let vcx = parsed.values();
        let parent = crate::style::computed::BoxStyle::default();
        let parents = crate::style::cascade::inherit::BoxParents::same(&parent);
        let a = super::ParentFacts::of(parents, vcx);
        let mut b = a.clone();
        assert_eq!(a.first_difference(&b), None);
        b.layout.height = Some(crate::style::values::value::Len::Px(10.0));
        assert_eq!(a.first_difference(&b), Some("layout.height"));
        let mut c = a.clone();
        c.tree.font_px += 1.0;
        assert_eq!(a.first_difference(&c), Some("tree.font_px"));
    }
}
