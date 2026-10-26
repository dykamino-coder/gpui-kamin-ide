//! Завершённая normal-flow геометрия, независимая от размещения при отрисовке.
//! Upstream proposal: https://github.com/zed-industries/zed/issues/66070.
use super::{AvailableSpace, LayoutId, TaffyLayoutEngine};
use crate::{Bounds, Pixels, Point, Size, point, size};
use collections::FxHashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Identity of a layout tree, including nested engines and frame clears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutTreeId(u64);

/// Identity of a completed root layout, even when its solver result was cached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutEpoch {
    /// Tree owning the completed pass.
    pub tree: LayoutTreeId,
    /// Monotonic completed-pass revision within the tree.
    pub revision: u64,
}

/// The actual inputs of a completed root pass, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutSpace {
    /// Identity of the producing pass.
    pub epoch: LayoutEpoch,
    /// Root submitted to the solver.
    pub root: LayoutId,
    /// Available space submitted to the solver, before device scaling.
    pub available: Size<AvailableSpace>,
    /// Device scale used for this pass.
    pub scale: f32,
}

/// Unrounded physical border box in tree coordinates, before visual translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinalLayout {
    /// Normal border box, excluding relative positioning and paint placement.
    pub normal: Bounds<Pixels>,
    /// Used content box, excluding resolved padding, borders and scrollbar gutters.
    pub content_size: Size<Pixels>,
    /// Accumulated used relative displacement. Transforms/paint placement are separate.
    pub relative_offset: Point<Pixels>,
    /// Constraints and epoch producing this geometry.
    pub space: LayoutSpace,
}

pub(super) struct FinalGeometry {
    tree: LayoutTreeId,
    revision: u64,
    roots: FxHashMap<LayoutId, LayoutSpace>,
}

impl Default for FinalGeometry {
    fn default() -> Self {
        static NEXT_TREE: AtomicU64 = AtomicU64::new(1);
        Self {
            tree: LayoutTreeId(NEXT_TREE.fetch_add(1, Ordering::Relaxed)),
            revision: 0,
            roots: FxHashMap::default(),
        }
    }
}

impl TaffyLayoutEngine {
    /// The identity must be captured when a layout ID is registered.
    pub fn layout_tree_id(&self) -> LayoutTreeId {
        self.final_geometry.tree
    }

    pub(super) fn begin_geometry(&mut self, root: LayoutId) {
        // Solving an attached subtree as a root replaces its placement. The next
        // ancestor pass must place it again, even if its constraints are unchanged.
        if let Some(parent) = self.taffy.parent(root.into()) {
            self.taffy.mark_dirty(parent).expect(super::EXPECT_MESSAGE);
        }
        let mut node = Some(root);
        while let Some(id) = node {
            self.final_geometry.roots.remove(&id);
            node = self.taffy.parent(id.into()).map(Into::into);
        }
    }

    pub(super) fn finish_geometry(&mut self, root: LayoutId, available: Size<AvailableSpace>, scale: f32) {
        let state = &mut self.final_geometry;
        state.revision += 1;
        state.roots.insert(root, LayoutSpace {
            epoch: LayoutEpoch { tree: state.tree, revision: state.revision },
            root,
            available,
            scale,
        });
    }

    /// KaminIDE patch: static rects are retained with the performed native leaf result.
    /// Dirty ancestry, intrinsic probes and copied measurements cannot publish geometry.
    pub fn final_static_position(&self, id: LayoutId, tree: LayoutTreeId, key: usize, scale: f32) -> Option<super::MeasuredStaticPosition> {
        self.final_layout(id, tree, scale)?;
        let mut position = self.taffy.get_node_context(id.into())?.static_positions.iter()
            .find(|position| position.key == key).copied()?;
        let layout = self.taffy.unrounded_layout(id.into());
        let content_origin = point(
            Pixels((layout.border.left + layout.padding.left) / scale),
            Pixels((layout.border.top + layout.padding.top) / scale),
        );
        position.rect.origin += content_origin;
        position.line.origin += content_origin;
        Some(position)
    }

    /// KaminIDE patch: the actual horizontal flex line containing an in-flow item.
    /// Unlike the item's own box this retains line height for zero-sized markers.
    pub fn final_flex_line(&self, id: LayoutId, tree: LayoutTreeId, scale: f32) -> Option<Bounds<Pixels>> {
        self.final_layout(id, tree, scale)?;
        let parent = self.taffy.parent(id.into())?;
        let parent_geometry = self.final_layout(parent.into(), tree, scale)?;
        let taffy::DetailedLayoutInfo::Flex(info) = self.taffy.detailed_layout_info(parent) else {
            return None;
        };
        if !matches!(info.direction, taffy::FlexDirection::Row | taffy::FlexDirection::RowReverse) {
            return None;
        }
        let line = info.lines.iter().find(|line| line.items.iter().any(|item| item.node == id.into()))?;
        let layout = self.taffy.unrounded_layout(parent);
        Some(Bounds {
            origin: parent_geometry.normal.origin + point(
                Pixels((layout.border.left + layout.padding.left) / scale),
                Pixels(line.cross_offset / scale),
            ),
            size: size(
                Pixels((layout.size.width - layout.border.left - layout.border.right - layout.padding.left - layout.padding.right) / scale),
                Pixels(line.cross_size / scale),
            ),
        })
    }

    /// Read only nodes belonging to a completed pass. An intrinsic cache entry
    /// is insufficient, and a dirty ancestor invalidates descendant placement.
    pub fn final_layout(&self, id: LayoutId, tree: LayoutTreeId, scale: f32) -> Option<FinalLayout> {
        if tree != self.final_geometry.tree {
            return None;
        }
        let mut node = id;
        let mut normal = point(Pixels::ZERO, Pixels::ZERO);
        let mut relative = normal;
        loop {
            self.taffy.computed_layout_output(node.into()).ok()??;
            let layout = self.taffy.unrounded_layout(node.into());
            normal.x += Pixels(layout.location.x - layout.relative_offset.x);
            normal.y += Pixels(layout.location.y - layout.relative_offset.y);
            relative.x += Pixels(layout.relative_offset.x);
            relative.y += Pixels(layout.relative_offset.y);
            match self.taffy.parent(node.into()) {
                Some(parent) => {
                    normal.y += Pixels(self.taffy.content_shift(parent));
                    node = parent.into();
                }
                None => break,
            }
        }
        let space = *self.final_geometry.roots.get(&node)?;
        if space.scale != scale {
            return None;
        }
        let layout = self.taffy.unrounded_layout(id.into());
        Some(FinalLayout {
            normal: Bounds {
                origin: normal / scale,
                size: size(Pixels(layout.size.width / scale), Pixels(layout.size.height / scale)),
            },
            content_size: size(
                Pixels((layout.content_box_width() - layout.scrollbar_size.width).max(0.0) / scale),
                Pixels((layout.content_box_height() - layout.scrollbar_size.height).max(0.0) / scale),
            ),
            relative_offset: relative / scale,
            space,
        })
    }
}

#[cfg(test)]
#[path = "static_position_tests.rs"]
mod static_position_tests;
