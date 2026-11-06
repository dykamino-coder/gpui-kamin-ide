//! KaminIDE patch: an exact leaf standing in for a laid out subtree.
//!
//! A host that skips building a settled subtree still needs its parent to
//! place the following siblings exactly where the full subtree placed them.
//! Only an inflexible, normal-flow item of a single-line column flex container
//! qualifies: its outer contribution there is its border-box size and margins
//! alone (CSS Flexbox 1 §9.7: an inflexible item is frozen at its hypothetical
//! main size; §9.4 a definite cross size is not stretched), or an in-flow
//! block-level box of a block container, which is placed by its margins and
//! border-box size (CSS2 §9.4.1, §10.3.3) once no descendant margin collapses
//! through its edges (§8.3.1). The leaf repeats
//! that size bit for bit, the margins and self-alignment, and contributes the
//! same scrollable overflow when the subtree's own overflow stays inside its
//! border box.

use super::{LayoutId, TaffyLayoutEngine};
use taffy::prelude::*;
use taffy::style::{BoxSizing, Direction, Overflow};

/// What an exact leaf repeats of the laid out node: its border-box size,
/// margins, self-alignment and the axes its parent reads. A few words instead
/// of a whole `Style`, so a host can keep one per settled subtree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ExactLeaf {
    display: Display,
    direction: Direction,
    block_flow: Option<taffy::style::BlockFlow>,
    size: Size<f32>,
    margin: Rect<LengthPercentageAuto>,
    align_self: Option<AlignSelf>,
    justify_self: Option<AlignSelf>,
    /// The width is the parent's stretch fit, not `size.width`.
    stretch: bool,
    /// Lower bound of a stretched width: the inline-end edge of the content
    /// of a box that paints nothing (`TaffyLayoutEngine::unpainted_leaf`).
    min_width: f32,
}

impl ExactLeaf {
    /// The border-box width, or for a box whose content overflows its
    /// inline-end edge (`unpainted_leaf`) the width up to that edge.
    pub(crate) fn width(&self) -> f32 {
        self.size.width.max(self.min_width)
    }

    pub(crate) fn stretched(self) -> Self {
        Self { stretch: true, ..self }
    }

    fn style(&self) -> Style {
        let size = self.size;
        let width = |definite: Dimension| if self.stretch { Dimension::auto() } else { definite };
        let limit = |definite: LengthPercentageAuto| {
            if self.stretch { LengthPercentageAuto::auto() } else { definite }
        };
        let floor = match self.stretch && self.min_width > 0.0 {
            true => LengthPercentageAuto::length(self.min_width),
            false => limit(LengthPercentageAuto::length(size.width)),
        };
        Style {
            display: self.display,
            box_sizing: BoxSizing::BorderBox,
            direction: self.direction,
            block_flow: self.block_flow,
            size: Size {
                width: width(Dimension::length(size.width)),
                height: Dimension::length(size.height),
            },
            min_size: Size {
                width: floor,
                height: LengthPercentageAuto::length(size.height),
            },
            max_size: Size {
                width: limit(LengthPercentageAuto::length(size.width)),
                height: LengthPercentageAuto::length(size.height),
            },
            margin: self.margin,
            align_self: self.align_self,
            justify_self: self.justify_self,
            flex_grow: 0.0,
            flex_shrink: 0.0,
            ..Style::default()
        }
    }
}

impl TaffyLayoutEngine {
    /// The exact leaf of the laid out node `id`, if it qualifies.
    pub(crate) fn exact_leaf(&self, id: LayoutId) -> Option<ExactLeaf> {
        self.leaf(id, false)
    }

    /// The exact leaf of the laid out node `id` that paints nothing of its
    /// own and whose content stays where it is at any width of the node (the
    /// caller's guarantee: an anonymous wrapper around content placed at its
    /// inline-start edge), if it qualifies. Its content may overflow its
    /// inline-end edge. Such a node is observable to its parent only through
    /// its place and its scrollable overflow, the union of its border box and
    /// its content (CSS Overflow 3 §2.2): a stretched leaf at least as wide as
    /// the content has the same place and the same union at every parent
    /// width at which the content is laid out the same.
    pub(crate) fn unpainted_leaf(&self, id: LayoutId) -> Option<ExactLeaf> {
        self.leaf(id, true)
    }

    fn leaf(&self, id: LayoutId, unpainted: bool) -> Option<ExactLeaf> {
        let node: NodeId = id.into();
        let layout = self.taffy.layout(node).ok()?;
        let style = self.taffy.style(node).ok()?;
        let parent = self.taffy.style(self.taffy.parent(node)?).ok()?;
        // A block container (Taffy block layout, or a column flex container
        // with block flow) places the item by its margins collapsed with
        // those of its descendants (CSS2 §8.3.1); the leaf has only its own,
        // so every descendant margin adjoining its edges must be zero.
        let block = parent.display == Display::Block;
        let column = block
            || parent.display == Display::Flex
                && parent.flex_direction == FlexDirection::Column
                && parent.flex_wrap == FlexWrap::NoWrap;
        let flow = match (block, parent.block_flow) {
            (false, None) => true,
            (_, flow) => {
                flow.is_none_or(|flow| !flow.vertical && !flow.block_reverse)
                    && self.own_margins(node, true)
                    && self.own_margins(node, false)
            }
        };
        let size = layout.size;
        let overflow = layout.scrollable_overflow_rect;
        let content_right = layout.border.left + overflow.right;
        let inside = overflow.left >= 0.0
            && overflow.top >= 0.0
            && (unpainted || content_right <= size.width)
            && layout.border.top + overflow.bottom <= size.height;
        let margins = [
            style.margin.left,
            style.margin.right,
            style.margin.top,
            style.margin.bottom,
        ];
        let normal = style.position == Position::Relative
            && style.inset == Rect::auto()
            && style.float == taffy::style::Float::None
            && style.clear == taffy::style::Clear::None
            && style.overflow.x == Overflow::Visible
            && style.overflow.y == Overflow::Visible
            && (block || style.flex_grow == 0.0 && style.flex_shrink == 0.0)
            && margins.iter().all(|m| !m.into_raw().has_calc());
        (column && flow && inside && normal && size.width > 0.0 && size.height > 0.0).then(|| ExactLeaf {
            display: style.display,
            direction: style.direction,
            block_flow: style.block_flow,
            size,
            margin: style.margin,
            align_self: style.align_self,
            justify_self: style.justify_self,
            stretch: unpainted,
            min_width: if unpainted { content_right.max(0.0) } else { 0.0 },
        })
    }

    /// Whether no descendant margin adjoins the top (`top`) or bottom edge of
    /// `node`: a border or padding separates them, or the first (last)
    /// in-flow child has a zero margin there, cannot collapse through and
    /// recursively has the same property.
    fn own_margins(&self, node: NodeId, top: bool) -> bool {
        let Ok(layout) = self.taffy.layout(node) else {
            return false;
        };
        let edge = if top {
            layout.border.top + layout.padding.top
        } else {
            layout.border.bottom + layout.padding.bottom
        };
        if edge > 0.0 {
            return true;
        }
        let Ok(children) = self.taffy.children(node) else {
            return false;
        };
        let in_flow = |child: &NodeId| {
            self.taffy.style(*child).is_ok_and(|style| {
                style.display != Display::None && style.position != Position::Absolute
            })
        };
        let child = if top {
            children.iter().find(|c| in_flow(c))
        } else {
            children.iter().rev().find(|c| in_flow(c))
        };
        let Some(&child) = child else {
            return layout.size.height > 0.0;
        };
        let (Ok(style), Ok(child_layout)) = (self.taffy.style(child), self.taffy.layout(child)) else {
            return false;
        };
        let margin = if top { style.margin.top } else { style.margin.bottom };
        let zero = margin.is_auto() || margin == LengthPercentageAuto::length(0.0);
        zero && child_layout.size.height > 0.0 && self.own_margins(child, top)
    }

    /// Add an exact leaf made by `exact_leaf` to this frame's tree. It is a
    /// plain leaf of the frame tree (`retained`): a leaf of an equal style at
    /// the same request position keeps its node and cache.
    pub(crate) fn request_exact_leaf(&mut self, leaf: &ExactLeaf) -> LayoutId {
        crate::frame_perf::LAYOUT_NODES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let style = self.styles.share(leaf.style());
        if let Some(id) = self.claim_leaf(&style) {
            return id;
        }
        let id = match self.recycle(false) {
            Some(node) => {
                self.taffy
                    .reset_node(node, style, &[], None)
                    .expect(super::EXPECT_MESSAGE);
                LayoutId(node)
            }
            None => self.taffy.new_leaf(style).expect(super::EXPECT_MESSAGE).into(),
        };
        self.created(id, None)
    }
}
