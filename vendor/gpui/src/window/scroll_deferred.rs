//! KaminIDE patch: deferred draws that belong to a scroll node.
//!
//! A deferred draw paints after the whole tree, in priority order, but its
//! content may still live in a scroll container's space: a positioned box
//! with a positive `z-index` or a sticky box lifted above the flow
//! (`Window::defer_scrolled_draw`). Such a draw keeps the scroll node that
//! was open when it was deferred (its owner) and prepaints and paints under
//! it again: its hitboxes and primitives are marked under the owner, clipped
//! by its chain, and its nested nodes are the owner's children. A scrolled
//! replay of a record takes each draw's prepaint and paint range as one more
//! part of the record and moves it with its owner as it moves the record
//! (`plan_scrolled_replay`), in the draw's own place of the
//! deferred pass: paint order and spatial node are separate, as Blink keeps
//! paint chunks in paint order with their own scroll translation and
//! WebRender primitives with their own spatial node.
//!
//! A plain `defer_draw` (a popover escaping its scroller) has no owner: a
//! record holding one is built instead of replayed scrolled.

use super::scroll_nodes::ScrollNodeRecord;
use super::scroll_replay::ScrollReplay;
use super::*;

/// The scroll node of a deferred draw, and the plan its copy replays with.
#[derive(Clone, Default)]
pub(crate) struct DeferredScroll {
    /// Deferred with `defer_scrolled_draw`.
    owned: bool,
    /// Index of the owner node in the draw's frame (`None`: under no node).
    pub(super) owner: Option<u32>,
    /// The interest margin its nodes record with (`with_scroll_interest_margin`
    /// where it was deferred): the deferred pass runs outside that scope.
    margin: Pixels,
    /// A scrolled replay copied the draw: the plan and the draw's position
    /// among the record's deferred draws (`ScrollReplay::deferred`).
    pub(super) plan: Option<(Rc<ScrollReplay>, usize)>,
}

/// Where `reuse_prepaint` of a range puts the owners of its deferred draws:
/// an owner inside the range is renumbered with the copied nodes, one
/// outside it is the node open where the range is copied, as
/// `reuse_scroll_nodes` renumbers parents.
#[derive(Clone)]
pub(super) struct OwnerMap {
    old: Range<usize>,
    first: usize,
    open: Option<u32>,
}

impl OwnerMap {
    pub(super) fn new(range: &Range<PrepaintStateIndex>, window: &Window) -> Self {
        Self {
            old: range.start.scroll_nodes_index..range.end.scroll_nodes_index,
            first: window.next_frame.scroll_nodes.len(),
            open: window.next_frame.scroll_node_stack.last().copied(),
        }
    }

    pub(super) fn copy(&self, scroll: &DeferredScroll) -> DeferredScroll {
        let owner = match scroll.owner {
            Some(owner) if self.old.contains(&(owner as usize)) => Some((self.first + owner as usize - self.old.start) as u32),
            Some(_) => self.open,
            None => None,
        };
        DeferredScroll { owned: scroll.owned, owner, margin: scroll.margin, plan: None }
    }
}

impl Window {
    /// The owner a draw deferred now gets: the innermost open prepaint node.
    pub(super) fn deferred_scroll_owner(&self) -> DeferredScroll {
        let owner = self.next_frame.scroll_node_stack.last().copied();
        DeferredScroll { owned: true, owner, margin: self.scroll_interest_margin, plan: None }
    }

    /// Open the owner of deferred draw `ix` for its prepaint: nodes it
    /// opens are the owner's children and record its margin, its hitboxes
    /// are marked under it. Returns the margin to restore.
    pub(super) fn enter_deferred_prepaint(&mut self, ix: usize) -> Option<Pixels> {
        let scroll = &self.next_frame.deferred_draws[ix].scroll;
        let (owner, margin) = (scroll.owned_node()?, scroll.margin);
        let interest = self.next_frame.scroll_nodes[owner as usize].interest;
        self.next_frame.scroll_node_stack.push(owner);
        self.open_interest(interest, false);
        Some(std::mem::replace(&mut self.scroll_interest_margin, margin))
    }

    /// Close what `enter_deferred_prepaint` opened.
    pub(super) fn leave_deferred_prepaint(&mut self, entered: Option<Pixels>) {
        if let Some(margin) = entered {
            self.scroll_interest_margin = margin;
            self.end_scroll_node_prepaint(Some(0));
        }
    }

    /// Prepaint of a deferred draw without its element: a copy of its
    /// rendered range, moved by the plan of the scrolled replay that copied
    /// the draw.
    pub(super) fn reuse_deferred_prepaint(&mut self, ix: usize, range: Range<PrepaintStateIndex>) {
        let plan = self.next_frame.deferred_draws[ix].scroll.plan.clone();
        let start = self.prepaint_index();
        self.reuse_prepaint(range.clone());
        if let Some((plan, at)) = plan {
            self.place_scrolled_prepaint(&start, plan.deferred[at].clone(), &plan);
            self.plan_deferred_copies(&start, &range, &plan);
        }
    }

    /// The draws `reuse_prepaint` of the rendered `range` just copied from
    /// `start` on replay with `plan`, each as the draw it copies.
    pub(super) fn plan_deferred_copies(&mut self, start: &PrepaintStateIndex, range: &Range<PrepaintStateIndex>, plan: &Rc<ScrollReplay>) {
        let first = range.start.deferred_draws_index;
        let copies = &mut self.next_frame.deferred_draws[start.deferred_draws_index..];
        for (at, draw) in copies.iter_mut().enumerate() {
            draw.scroll.plan = plan.draws.get(&(first + at)).map(|&part| (plan.clone(), part));
        }
    }

    /// Open the owner of a deferred draw for its paint: its primitives are
    /// marked under it (`with_scene_node`), with its interest rect as their
    /// own mask and its clip chain as what they draw in.
    pub(super) fn enter_deferred_paint(&mut self, scroll: &DeferredScroll) -> bool {
        let Some(owner) = scroll.owned_node() else { return false };
        let interest = self.next_frame.scroll_nodes[owner as usize].interest;
        self.open_interest(interest, true);
        let depth = self.content_mask_stack.len() - 1;
        self.next_frame.scroll_paint.open.push((owner, depth));
        true
    }

    /// Close what `enter_deferred_paint` opened. The draw's commands stay
    /// the draw's range (`paint_range`), not one of the owner's: the owner
    /// may have painted in a scene layer, the deferred pass paints in the
    /// frame's scene, and a scrolled replay checks and moves that range
    /// itself (`deferred_scroll_part`).
    pub(super) fn leave_deferred_paint(&mut self, entered: bool) {
        if entered {
            self.close_interest(true);
        }
    }

    /// Paint of a deferred draw without its element: a copy of its rendered
    /// commands, moved by the plan of the scrolled replay that copied it,
    /// with the composition `groups` it made in the rendered frame copied
    /// and its labels renumbered, as a record's (`replay_groups`).
    pub(super) fn reuse_deferred_paint(&mut self, scroll: &mut DeferredScroll, range: Range<PaintIndex>, groups: Range<usize>) {
        let scenes = range.start.scene_index..range.end.scene_index;
        match scroll.plan.take() {
            None => {
                self.replay_groups(groups, None);
                self.reuse_paint(range);
            }
            Some((plan, _)) => {
                self.reuse_paint_state(&range);
                let scale = self.scale_factor();
                let node = |id: ScrollNodeId| plan.scaled(id, scale);
                let labels = self.rendered_frame.scene.group_labels(scenes.clone(), groups.clone());
                self.replay_groups(groups, Some((&labels, &node)));
                self.replay_scenes_scrolled(scenes, &plan);
            }
        }
        self.next_frame.scene.relabel = 0;
    }

    /// What a deferred draw of a record adds to the record for a scrolled
    /// replay: its prepainted nodes, its paint range and the draws nested in
    /// it (by index). `DeferredDraws` when the copy could not be placed: a
    /// draw without an owner node (`defer_draw`), with an own mask or of an
    /// isolated view. The planner checks the owner is planned before it.
    pub(super) fn deferred_scroll_part(
        &self,
        draw: &DeferredDraw,
    ) -> Result<(Range<usize>, Range<usize>, Range<usize>), ScrollRefusal> {
        let (prepaint, paint) = (&draw.prepaint_range, &draw.paint_range);
        if draw.scroll_refusal().is_some() {
            return Err(ScrollRefusal::DeferredDraws);
        }
        let nodes = prepaint.start.scroll_nodes_index..prepaint.end.scroll_nodes_index;
        let nested = prepaint.start.deferred_draws_index..prepaint.end.deferred_draws_index;
        Ok((nodes, paint.start.scene_index..paint.end.scene_index, nested))
    }

    /// Why a deferred draw of the rendered record `range` (its own draws,
    /// then the draws nested in them, as `plan_scrolled_replay` takes them)
    /// refuses a scrolled replay by its commands, for DevTools: the first
    /// draw whose paint range holds a command no node marks, the line that
    /// deferred it, its element, its owner node and the command
    /// (`Scene::scroll_refusal_detail`).
    pub(super) fn deferred_refusal_detail(&self, range: &Range<PrepaintStateIndex>) -> Option<String> {
        let frame = &self.rendered_frame;
        let space = frame.scene.scroll.space;
        let mut draws: Vec<usize> = (range.start.deferred_draws_index..range.end.deferred_draws_index).collect();
        let mut next = 0;
        while let Some(&index) = draws.get(next) {
            next += 1;
            let Some(draw) = frame.deferred_draws.get(index) else { continue };
            let (prepaint, paint) = (&draw.prepaint_range, &draw.paint_range);
            draws.extend(prepaint.start.deferred_draws_index..prepaint.end.deferred_draws_index);
            let painted = paint.start.scene_index..paint.end.scene_index;
            let nodes = frame.scroll_nodes.get(prepaint.start.scroll_nodes_index..prepaint.end.scroll_nodes_index);
            let nodes = nodes.unwrap_or_default().iter().filter(|node| node.painted_in == Some(space));
            let Some(detail) = frame.scene.scroll_refusal_detail(std::iter::once(painted).chain(nodes.flat_map(|node| node.ranges()))) else {
                continue;
            };
            let owner = draw.scroll.owner.and_then(|at| frame.scroll_nodes.get(at as usize)).map(|node| node.id);
            let element: Vec<String> = draw.element_id_stack.iter().rev().take(3).map(|id| format!("{id:?}")).collect();
            return Some(format!(
                "deferred draw {index} (priority {}) deferred at {}:{} under {}, owner node {owner:?}: {detail}",
                draw.priority,
                draw.site.file().replace('\\', "/"),
                draw.site.line(),
                element.join(" < ")
            ));
        }
        None
    }

    /// The nodes of the rendered frame's deferred draws registered in
    /// `range` (a record's prepaint range).
    pub(super) fn deferred_scroll_nodes(&self, range: &Range<PrepaintStateIndex>) -> impl Iterator<Item = &ScrollNodeRecord> {
        let frame = &self.rendered_frame;
        frame.deferred_draws[range.start.deferred_draws_index..range.end.deferred_draws_index]
            .iter()
            .flat_map(|draw| &frame.scroll_nodes[draw.prepaint_range.start.scroll_nodes_index..draw.prepaint_range.end.scroll_nodes_index])
    }
}

impl DeferredDraw {
    /// Why a scrolled replay cannot take this draw (`deferred_scroll_part`),
    /// for DevTools; `None` when it can.
    pub(super) fn scroll_refusal(&self) -> Option<&'static str> {
        match () {
            _ if !self.scroll.owned => Some("no scroll node (a plain defer_draw)"),
            _ if self.content_mask.is_some() => Some("its own content mask"),
            _ if self.isolated.is_some() => Some("an isolated view"),
            _ => None,
        }
    }
}

impl DeferredScroll {
    fn owned_node(&self) -> Option<u32> {
        self.owned.then_some(self.owner).flatten()
    }
}

#[cfg(test)]
mod tests;
