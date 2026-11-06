//! Resolve a box's physical preferred size and ratio-transferred bounds once.
use crate::tree::LayoutPartialTreeExt;
use crate::{BoxSizing, CoreStyle, LayoutPartialTree, MaybeMath, MaybeResolve, NodeId, ResolveOrZero, Size};

/// Resolved physical border-box constraints shared by root and atomic layout.
pub(crate) struct BoxConstraints {
    /// Preferred border-box size after aspect-ratio transfer.
    pub preferred: Size<Option<f32>>,
    /// Resolved minimum border-box size.
    pub minimum: Size<Option<f32>>,
    /// Resolved maximum border-box size.
    pub maximum: Size<Option<f32>>,
    /// Combined padding and border on each physical axis.
    pub padding_border: Size<f32>,
    /// Padding and border added to content-box size declarations.
    pub adjustment: Size<f32>,
    /// Preferred width-to-height ratio, if specified.
    pub aspect_ratio: Option<f32>,
}

impl BoxConstraints {
    /// Return dimensions fixed by preferred sizes or coincident min/max bounds.
    pub fn known(&self) -> Size<Option<f32>> {
        let bounded = self.minimum.zip_map(self.maximum, |min, max| match (min, max) {
            (Some(min), Some(max)) if max <= min => Some(min),
            _ => None,
        });
        bounded.or(self.preferred).maybe_clamp(self.minimum, self.maximum).maybe_max(self.padding_border)
    }
}

/// Resolve sizes against the parent and percentage edges against its inline basis.
pub(crate) fn resolve(
    tree: &impl LayoutPartialTree,
    node: NodeId,
    parent: Size<Option<f32>>,
    inline_basis: Option<f32>,
) -> BoxConstraints {
    let style = tree.get_core_container_style(node);
    let padding = style.padding().resolve_or_zero(inline_basis, |v, basis| tree.calc(v, basis));
    let border = style.border().resolve_or_zero(inline_basis, |v, basis| tree.calc(v, basis));
    let padding_border = (padding + border).sum_axes();
    let adjustment = if style.box_sizing() == BoxSizing::ContentBox { padding_border } else { Size::ZERO };
    let preferred = style.size().maybe_resolve(parent, |v, basis| tree.calc(v, basis));
    let (minimum, maximum) = super::aspect_ratio_constraints::transfer(
        preferred,
        style.min_size().maybe_resolve(parent, |v, basis| tree.calc(v, basis)),
        style.max_size().maybe_resolve(parent, |v, basis| tree.calc(v, basis)),
        style.aspect_ratio(),
    );
    BoxConstraints {
        preferred: preferred.maybe_apply_aspect_ratio(style.aspect_ratio()).maybe_add(adjustment),
        minimum: minimum.maybe_add(adjustment),
        maximum: maximum.maybe_add(adjustment),
        padding_border,
        adjustment,
        aspect_ratio: style.aspect_ratio(),
    }
}
