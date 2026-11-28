//! KaminIDE patch: replay of a retained record with its scroll nodes at new
//! offsets (`scroll_nodes`, `scroll_masks`, `scene::scroll_marks`).
//!
//! A wheel step that only moves a scroll offset needs no new build: the
//! record's commands under each node move by the node's shift and are drawn
//! with (own mask + shift) ∩ the node's new clip chain, its hitboxes the same,
//! as cc moves a scroll layer and WebRender a scroll frame. The replay is
//! allowed while each node's new clip lies inside the region its content was
//! recorded for (its interest rect, moved with it); otherwise the host builds.

use super::*;
use crate::scene::LayerHandle;
use collections::FxHashMap;

/// A planned scrolled replay: per node of the record, in record order, its
/// new offset, shift, real clip chain and interest rect; then the nodes of
/// the record's deferred draws (`scroll_deferred`), draw by draw.
#[derive(Clone, Default)]
pub struct ScrollReplay {
    pub(super) nodes: Vec<PlannedNode>,
    pub(super) by_id: FxHashMap<ScrollNodeId, usize>,
    /// How many of `nodes` the record's own prepaint range holds.
    pub(super) main: usize,
    /// The slots in `nodes` of each deferred draw's nodes.
    pub(super) deferred: Vec<Range<usize>>,
    /// The draw of `deferred` each rendered deferred draw of the record is
    /// (by its index in the rendered frame), nested ones too.
    pub(super) draws: FxHashMap<usize, usize>,
}

#[derive(Clone, Copy)]
pub(super) struct PlannedNode {
    offset: Point<Pixels>,
    /// Shift of the node's children since the record (window pixels).
    shift: Point<Pixels>,
    /// The shift is whole device pixels (not a follower's own function).
    whole: bool,
    clip: Bounds<Pixels>,
    chain: Bounds<Pixels>,
    interest: Bounds<Pixels>,
    visibility_changed: bool,
}

impl ScrollReplay {
    /// Whether nothing moves (a plain replay draws the same).
    pub fn is_still(&self) -> bool {
        self.nodes.iter().all(|node| node.shift == Point::default() && !node.visibility_changed)
    }

    pub(super) fn node(&self, id: ScrollNodeId) -> &PlannedNode {
        &self.nodes[self.by_id[&id]]
    }

    /// A planned node's shift and clip in scene pixels (`Scene::replay_scrolled`).
    pub(super) fn scaled(&self, id: ScrollNodeId, scale: f32) -> (Point<ScaledPixels>, Bounds<ScaledPixels>) {
        let node = self.node(id);
        let shift = node.shift.scale(scale);
        let shift = shift.map(|v| if node.whole || (v.0 - v.0.round()).abs() < 1e-3 { ScaledPixels(v.0.round()) } else { v });
        (shift, node.clip.scale(scale))
    }
}

fn moved(bounds: Bounds<Pixels>, shift: Point<Pixels>) -> Bounds<Pixels> {
    Bounds { origin: bounds.origin + shift, size: bounds.size }
}

fn inside(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.is_empty() || inner.intersect(&outer) == inner
}

impl Window {
    /// Whether a scroll node of a record made in the rendered frame is at
    /// another offset now (its scroll handle, or its anchor's function).
    pub fn retained_scrolled(&self, prepaint: &RetainedPrepaint) -> bool {
        let range = prepaint.range();
        self.rendered_frame.scroll_nodes[range.start.scroll_nodes_index..range.end.scroll_nodes_index]
            .iter()
            .chain(self.deferred_scroll_nodes(&range))
            .any(|node| node.source.offset(self.scale_factor()) != node.offset || node.source.visible() != node.visible)
    }

    /// Plan the replay of a record made in the rendered frame with each of its
    /// scroll nodes at its offset now (`retained_scrolled`). `None` when the
    /// record must be built instead: a node's new clip shows content outside
    /// what was recorded, or the record holds commands a moved copy cannot
    /// place (unmarked copies, layers, depth sorting, deferred draws without
    /// an owner node, `scroll_deferred`).
    ///
    /// `layer`: the record's paint was made inside that scene layer (`paint`
    /// indexes its scene), and the replay paints a new layer from it
    /// (`replay_retained_paint_scrolled` while that layer is the source).
    pub fn plan_scrolled_replay(
        &self,
        prepaint: &RetainedPrepaint,
        paint: &RetainedPaint,
        layer: Option<&LayerHandle>,
    ) -> Option<ScrollReplay> {
        self.scroll_replay_plan(prepaint, paint, layer).ok()
    }

    /// `plan_scrolled_replay` with the reason a record must be built.
    pub fn scroll_replay_plan(
        &self,
        prepaint: &RetainedPrepaint,
        paint: &RetainedPaint,
        layer: Option<&LayerHandle>,
    ) -> Result<ScrollReplay, ScrollRefusal> {
        self.located_scroll_plan(prepaint, paint, layer).map_err(|(refusal, _, _)| refusal)
    }

    /// Why `scroll_replay_plan` refuses, for DevTools: the refusal, the line
    /// that decided it and what it saw (the deferred draws' sites, the node).
    /// Plans again; call it only to explain a refusal.
    pub fn explain_scroll_refusal(
        &self,
        prepaint: &RetainedPrepaint,
        paint: &RetainedPaint,
        layer: Option<&LayerHandle>,
    ) -> Option<(ScrollRefusal, &'static core::panic::Location<'static>, String)> {
        let (refusal, site, refused) = self.located_scroll_plan(prepaint, paint, layer).err()?;
        let range = prepaint.range();
        let detail = match refusal {
            ScrollRefusal::DeferredDraws => {
                let draws = &self.rendered_frame.deferred_draws;
                let draws = draws.get(range.start.deferred_draws_index..range.end.deferred_draws_index);
                let draws = draws.unwrap_or_default();
                // One line per refusing site, owner and reason, with its count
                // (`DeferredDraw::scroll_refusal`); draws a replay takes are not listed.
                let mut sites = std::collections::BTreeMap::<String, usize>::new();
                for draw in draws {
                    let Some(why) = draw.scroll_refusal() else { continue };
                    let owner = draw.element_id_stack.last().map(|id| format!(" under {id:?}")).unwrap_or_default();
                    let site = format!("{}:{}{owner}: {why}", draw.site.file().replace('\\', "/"), draw.site.line());
                    *sites.entry(site).or_default() += 1;
                }
                if sites.is_empty() {
                    // Every draw is owned: a nested draw's owner is planned after it.
                    return Some((refusal, site, format!("{} deferred draws in the record; a draw's owner node comes after it", draws.len())));
                }
                let sites: Vec<String> = sites.into_iter().map(|(site, n)| format!("{n} x {site}")).collect();
                format!("{} deferred draws in the record, refused: {}", draws.len(), sites.join("; "))
            }
            ScrollRefusal::Transform => match refused.and_then(|at| self.rendered_frame.scroll_nodes.get(at)) {
                Some(node) => format!(
                    "scroll node {:?} (frame node {}) moves a primitive drawn through a matrix a shift cannot move exactly, without the inputs of that matrix (painted under another transform, or copied from a record)",
                    node.id,
                    refused.unwrap_or_default()
                ),
                None => String::new(),
            },
            ScrollRefusal::Fractional => match refused.and_then(|at| self.rendered_frame.scroll_nodes.get(at)) {
                Some(node) => format!(
                    "scroll node {:?} (frame node {}) moves by a fraction of a device pixel: offset {:?} -> {:?}",
                    node.id,
                    refused.unwrap_or_default(),
                    node.offset,
                    node.source.offset(self.scale_factor())
                ),
                None => String::new(),
            },
            ScrollRefusal::Node => {
                // The node the plan refused, wherever it is (the record's, or a
                // deferred draw's), and what decided it.
                let Some(node) = refused.and_then(|at| self.rendered_frame.scroll_nodes.get(at)) else {
                    return Some((refusal, site, String::new()));
                };
                let content = layer.map(LayerHandle::content);
                let source = content.as_deref().map_or(&self.rendered_frame.scene, |layer| layer.scene());
                let groups = source.groups.get(paint.groups()).unwrap_or_default();
                let scene = match node.painted_in {
                    None => "no paint".to_string(),
                    Some(space) if space == source.scroll.space => "the record's scene".into(),
                    Some(space) if space == self.rendered_frame.scene.scroll.space => "the frame's scene".into(),
                    Some(space) => match groups.iter().position(|group| group.scene.scroll.space == space) {
                        Some(at) => format!("the scene of group {at} of {}", groups.len()),
                        None => self.describe_space(space),
                    },
                };
                let why = if let Some(site) = node.torn {
                    format!("torn at {}:{}", site.file(), site.line())
                } else if node.paint.is_none() {
                    "no paint, and it moves apart from its parent into a clip that shows".into()
                } else {
                    "painted in a scene the part does not own".into()
                };
                format!(
                    "scroll node {:?} (frame node {}): {why}; painted in {scene}, anchored {}, offset {:?} -> {:?}, chain {:?}, clip {:?}, parent {:?}",
                    node.id,
                    refused.unwrap_or_default(),
                    node.anchored,
                    node.offset,
                    node.source.offset(self.scale_factor()),
                    node.chain,
                    node.clip,
                    node.parent
                )
            }
            ScrollRefusal::Unmarked
            | ScrollRefusal::UnmarkedLayer
            | ScrollRefusal::ForeignMark
            | ScrollRefusal::RetainedLayer
            | ScrollRefusal::LayerReservoir => {
                // The record's own nodes in its scene, then its deferred draws'.
                let nodes = &self.rendered_frame.scroll_nodes[range.start.scroll_nodes_index..range.end.scroll_nodes_index];
                let content = layer.map(LayerHandle::content);
                let source = content.as_deref().map_or(&self.rendered_frame.scene, |layer| layer.scene());
                source
                    .scroll_refusal_detail(nodes.iter().flat_map(|node| node.ranges()))
                    .or_else(|| self.deferred_refusal_detail(&range))
                    .unwrap_or_else(|| "in a deferred draw of the record".into())
            }
            ScrollRefusal::Groups => {
                let content = layer.map(LayerHandle::content);
                let source = content.as_deref().map_or(&self.rendered_frame.scene, |layer| layer.scene());
                let groups = paint.groups();
                let scenes = paint.range().start.scene_index..paint.range().end.scene_index;
                let labels = source.group_labels(scenes.clone(), groups.clone());
                // The first group whose scene a moved copy cannot take, and why.
                let refused = source.groups.get(groups.clone()).unwrap_or_default().iter().zip(&labels).enumerate().find_map(
                    |(at, (group, label))| {
                        let scene = &group.scene;
                        let all = 0..scene.paint_operations.len();
                        let why = match label {
                            crate::scene::GroupLabel::Unmarked if !scene.scroll.marks.is_empty() => {
                                // Where the group is and whose commands it holds, so the
                                // journal ties it to the nodes listed for the frame.
                                let mut nodes: Vec<_> = scene.scroll.marks.iter().map(|mark| mark.node).collect();
                                nodes.dedup();
                                nodes.truncate(4);
                                // Where its label is: in the record's commands, in a group's, or neither.
                                let number = (groups.start + at + 1) as u32;
                                use crate::scene::{PaintOperation as Op, Primitive as P};
                                let label_of = |scene: &crate::scene::Scene| {
                                    scene.paint_operations.iter().enumerate().find_map(|(op, operation)| {
                                        let kind = match operation {
                                            Op::Primitive(P::Surface(s)) if s.group == number => "drawn",
                                            Op::Dormant(p) if matches!(&**p, P::Surface(s) if s.group == number) => "dormant",
                                            Op::Reservoir(r) if r.run.items.iter().any(|item| matches!(&item.piece, crate::scene::scroll_marks::Piece::Primitive(P::Surface(s)) if s.group == number)) => "in a reservoir",
                                            _ => return None,
                                        };
                                        let marked = scene.scroll.marks.iter().any(|mark| mark.op as usize == op);
                                        Some(format!("{kind} label at command {op} (marked {marked})"))
                                    })
                                };
                                let found = match label_of(source) {
                                    Some(op) => format!("{op} of the frame's scene (record {scenes:?})"),
                                    None => source.groups.iter().enumerate().find_map(|(k, g)| label_of(&g.scene).map(|op| format!("{op} of group {k} (its label {:?})", labels.get(k.wrapping_sub(groups.start)))))
                                        .unwrap_or_else(|| "label in no command".into()),
                                };
                                format!("its label is under no node ({found}), its commands are: bounds {:?}, nodes {nodes:?}", group.bounds)
                            }
                            crate::scene::GroupLabel::Unmarked | crate::scene::GroupLabel::Hidden => return None,
                            crate::scene::GroupLabel::Under(_) => match scene.scroll_check(&all, std::iter::once(all.clone())) {
                                Ok(()) => return None,
                                Err(_) => scene.scroll_refusal_detail(std::iter::once(all.clone())).unwrap_or_default(),
                            },
                        };
                        Some(format!("group {at} of {} (label node {label:?}): {why}", labels.len()))
                    },
                );
                let layered = if layer.is_some() { "painted in a scene layer; " } else { "" };
                format!("{layered}{}", refused.unwrap_or_else(|| "a group's marks name a node the plan does not know".into()))
            }
            _ => String::new(),
        };
        Some((refusal, site, detail))
    }

    fn located_scroll_plan(
        &self,
        prepaint: &RetainedPrepaint,
        paint: &RetainedPaint,
        layer: Option<&LayerHandle>,
    ) -> Result<ScrollReplay, (ScrollRefusal, &'static core::panic::Location<'static>, Option<usize>)> {
        #[track_caller]
        fn at(refusal: ScrollRefusal) -> (ScrollRefusal, &'static core::panic::Location<'static>, Option<usize>) {
            (refusal, core::panic::Location::caller(), None)
        }
        // A refusal by one node names it (`explain_scroll_refusal`).
        #[track_caller]
        fn at_node(refusal: ScrollRefusal, index: usize) -> (ScrollRefusal, &'static core::panic::Location<'static>, Option<usize>) {
            (refusal, core::panic::Location::caller(), Some(index))
        }
        let (prepaints, paints) = (prepaint.range(), paint.range());
        let scene = &self.next_frame.scene;
        if !paint.replayable
            || self.replay_source().is_some()
            || scene.segment_capture.is_some()
            || !scene.at_root()
        {
            return Err(at(ScrollRefusal::Environment));
        }
        let content = layer.map(LayerHandle::content);
        let source = content.as_deref().map_or(&self.rendered_frame.scene, |layer| layer.scene());
        let scenes = paints.start.scene_index..paints.end.scene_index;
        // The record's nodes and commands in its scene, then those of each
        // deferred draw it registered (`scroll_deferred`), in the frame's scene:
        // the deferred pass paints outside the layer the record may paint in.
        let main = prepaints.start.scroll_nodes_index..prepaints.end.scroll_nodes_index;
        let mut parts = vec![(main.clone(), scenes.clone(), source, paint.groups())];
        // Draws nested in a draw (registered while it prepainted) follow it.
        let mut owners = vec![None];
        let mut draws: Vec<usize> = (prepaints.start.deferred_draws_index..prepaints.end.deferred_draws_index).collect();
        let mut next = 0;
        let mut plan = ScrollReplay::default();
        while let Some(&index) = draws.get(next) {
            let draw = &self.rendered_frame.deferred_draws[index];
            let (nodes, painted, nested) = self.deferred_scroll_part(draw).map_err(|refusal| at(refusal))?;
            plan.draws.insert(index, parts.len() - 1);
            parts.push((nodes, painted, &self.rendered_frame.scene, draw.groups.clone()));
            owners.push(Some(draw.scroll.owner));
            draws.extend(nested);
            next += 1;
        }
        plan.main = main.len();
        let mut slots = FxHashMap::<u32, usize>::default();
        // Nodes replayed unmoved in a retained layer the parts draw.
        let mut layered_ids = Vec::new();
        for (part, (indices, span, scene, groups)) in parts.iter().enumerate() {
            // A node painted inside one of the part's composition groups has
            // its ranges in the group's scene: the group's copy moves its
            // commands (`groups_scrollable` checks them) and lands it (`replay_groups`).
            let grouped = scene.groups.get(groups.clone()).unwrap_or_default();
            let group_of = |node: &super::scroll_nodes::ScrollNodeRecord| grouped.iter().find(|group| Some(group.scene.scroll.space) == node.painted_in);
            if let Some(owner) = owners[part] {
                // The owner moves the draw: it must be planned before it.
                if owner.is_some_and(|owner| !slots.contains_key(&owner)) {
                    return Err(at(ScrollRefusal::DeferredDraws));
                }
                plan.deferred.push(plan.nodes.len()..plan.nodes.len() + indices.len());
            }
            let mut drawn = None;
            for (index, node) in indices.clone().zip(&self.rendered_frame.scroll_nodes[indices.clone()]) {
                // Every paint range of the node in its part's scene, deferred ones too,
                // or in the scene of a retained layer the part's commands draw: the copy
                // draws the layer as it was, so the node replays there only unmoved, and
                // the layer only when all it holds stays (`still_layers`).
                let painted = node.paint.is_some();
                let group = group_of(node);
                let foreign = painted && node.painted_in != Some(scene.scroll.space) && group.is_none();
                let layered = foreign
                    && node.painted_in.is_some_and(|space| {
                        let drawn = drawn.get_or_insert_with(|| super::scroll_nodes::layer_spaces(&scene.paint_operations[span.clone()]));
                        drawn.contains(&space)
                    });
                if (foreign && !layered) || node.torn.is_some() {
                    return Err(at_node(ScrollRefusal::Node, index));
                }
                let span = group.map_or(span.clone(), |group| 0..group.scene.paint_operations.len());
                if !layered && !node.ranges().all(|painted| painted.start >= span.start && painted.end <= span.end) {
                    return Err(at(ScrollRefusal::NodeRange));
                }
                let new = node.source.offset(self.scale_factor());
                let parent = node.parent.and_then(|parent| slots.get(&parent)).map(|&parent| plan.nodes[parent]);
                let parent_shift = parent.map_or(Point::default(), |parent| parent.shift);
                // An anchor's mask stays in the viewport; a node's moves with its parent.
                let chain = if node.anchored { node.chain } else { moved(node.chain, parent_shift) };
                let visible = node.source.visible();
                let clip = match parent {
                    Some(parent) => parent.clip.intersect(&chain),
                    None => chain,
                };
                let clip = if visible { clip } else { Bounds::default() };
                let shift = node.source.replay_shift(parent_shift, node.offset, new);
                // A matrix without its pivot is drawn only where it was built.
                if node.unpivoted && shift != Point::default() {
                    return Err(at_node(ScrollRefusal::Transform, index));
                }
                // A move by a fraction of a device pixel lands its paint between
                // the pixels a fresh paint there snaps to: it is built. A
                // follower (a scrollbar thumb, a sticky box) is placed by its
                // own function of the offset, unsnapped, so it moves exactly.
                let device = shift.scale(self.scale_factor());
                let follower = matches!(node.source, scroll_nodes::NodeSource::Follow { .. });
                // Painted offsets are whole device pixels (`ScrollHandle::painted`)
                // up to the f32 error of their CSS values.
                let off = |v: f32| (v - v.round()).abs() > 1e-3;
                if !follower && (off(device.x.0) || off(device.y.0)) {
                    return Err(at_node(ScrollRefusal::Fractional, index));
                }
                // A node whose children painted nothing as it (hidden, or painted
                // under the node open around them) is replayed only moving with its
                // parent: whatever it shows then moves as its parent's commands.
                // Moving apart, it shows nothing while its clip stays empty (an
                // anchor past the viewport edge, whose children were never painted).
                if (!painted && shift != parent_shift && !clip.is_empty()) || (layered && (shift != Point::default() || clip != node.clip)) {
                    return Err(at_node(ScrollRefusal::Node, index));
                }
                let interest = moved(node.interest, shift);
                if !inside(clip, interest) {
                    return Err(at(ScrollRefusal::Interest));
                }
                if layered {
                    layered_ids.push(node.id);
                }
                slots.insert(index as u32, plan.nodes.len());
                plan.by_id.insert(node.id, plan.nodes.len());
                plan.nodes.push(PlannedNode { offset: new, shift, whole: !follower, clip, chain, interest, visibility_changed: visible != node.visible });
            }
        }
        // Paint that follows a moved node's offset outside it (a thumb).
        let moved = |id: ScrollNodeId| plan.by_id.get(&id).is_some_and(|&at| plan.nodes[at].shift != Point::default());
        // In the part's scene, or in the scene of one of its groups.
        let dependents = |(_, span, scene, groups): &(_, Range<usize>, &crate::scene::Scene, Range<usize>)| {
            self.scroll_dependents_moved(scene.scroll.space, span, moved)
                || scene.groups.get(groups.clone()).unwrap_or_default().iter().any(|group| {
                    self.scroll_dependents_moved(group.scene.scroll.space, &(0..group.scene.paint_operations.len()), moved)
                })
        };
        if parts.iter().any(dependents) {
            return Err(at(ScrollRefusal::Dependent));
        }
        // The groups the record made move with their labels' nodes (`group_replay`).
        let groups = paint.groups();
        if !groups.is_empty() {
            let labels = source.group_labels(scenes.clone(), groups.clone());
            if layer.is_some() || !source.groups_scrollable(groups, &labels, |id| plan.by_id.contains_key(&id)) {
                return Err(at(ScrollRefusal::Groups));
            }
        }
        // So do those of its deferred draws, in the frame's scene
        // (`reuse_deferred_paint`).
        let frame = &self.rendered_frame;
        for draw in plan.draws.keys().map(|&index| &frame.deferred_draws[index]) {
            let painted = draw.paint_range.start.scene_index..draw.paint_range.end.scene_index;
            let groups = draw.groups.clone();
            if !groups.is_empty() {
                let labels = frame.scene.group_labels(painted, groups.clone());
                if !frame.scene.groups_scrollable(groups, &labels, |id| plan.by_id.contains_key(&id)) {
                    return Err(at(ScrollRefusal::Groups));
                }
            }
        }
        let still = super::scroll_still::still_layers(&layered_ids, parts.iter().map(|(_, span, scene, _)| &scene.paint_operations[span.clone()]));
        for (part, (indices, span, scene, _)) in parts.iter().enumerate() {
            // Nodes painted in a group are checked with its scene (`groups_scrollable`).
            let nodes = self.rendered_frame.scroll_nodes[indices.clone()].iter();
            let nodes = nodes.filter(|node| node.painted_in == Some(scene.scroll.space)).flat_map(|node| node.ranges());
            // A deferred draw's commands are all under its owner or its nodes.
            let whole = (part > 0).then(|| span.clone());
            // The scene's check (`scene::scroll_marks::check`) decided: its caller is the site.
            scene.scroll_check_still(span, whole.into_iter().chain(nodes), &|space| still.contains(&space)).map_err(|refusal| at(refusal))?;
        }
        Ok(plan)
    }

    /// `replay_retained_prepaint` of a planned scrolled replay: hitboxes
    /// under a node move by its shift and are hit-tested with (own + shift)
    /// ∩ its new clip; the nodes take their new offsets and clips.
    pub fn replay_retained_prepaint_scrolled(
        &mut self,
        record: &RetainedPrepaint,
        plan: &ScrollReplay,
    ) -> RetainedPrepaint {
        let start = self.prepaint_index();
        let range = record.range();
        self.reuse_prepaint(range.clone());
        self.place_scrolled_prepaint(&start, 0..plan.main, plan);
        // The copies of the record's deferred draws replay with the plan too.
        if !plan.deferred.is_empty() {
            self.plan_deferred_copies(&start, &range, &std::rc::Rc::new(plan.clone()));
        }
        RetainedPrepaint::of(start..self.prepaint_index())
    }

    /// The nodes and hitboxes a reused prepaint range copied from `start` on
    /// take the planned offsets, clips and shifts (`slots` of the plan).
    pub(super) fn place_scrolled_prepaint(&mut self, start: &PrepaintStateIndex, slots: Range<usize>, plan: &ScrollReplay) {
        let frame = &mut self.next_frame;
        for (node, planned) in frame.scroll_nodes[start.scroll_nodes_index..].iter_mut().zip(&plan.nodes[slots]) {
            node.offset = planned.offset;
            node.visible = node.source.visible();
            // KaminIDE patch: the children landed moved by the plan (`scroll_step`).
            node.children_offset += planned.shift;
            node.clip = planned.clip;
            node.chain = planned.chain;
            node.interest = planned.interest;
        }
        let from = frame.scroll_hitboxes.partition_point(|mark| (mark.index as usize) < start.hitboxes_index);
        for mark in &mut frame.scroll_hitboxes[from..] {
            let Some(&at) = plan.by_id.get(&mark.node) else { continue };
            let planned = plan.nodes[at];
            mark.own.origin += planned.shift;
            let hitbox = &mut frame.hitboxes[mark.index as usize];
            hitbox.bounds.origin += planned.shift;
            hitbox.content_mask.bounds = mark.own.intersect(&planned.clip);
        }
    }

    /// `replay_retained_paint` of a planned scrolled replay
    /// (`Scene::replay_scrolled`).
    pub fn replay_retained_paint_scrolled(&mut self, record: &RetainedPaint, plan: &ScrollReplay) -> RetainedPaint {
        let start = self.paint_index();
        let range = record.range();
        self.reuse_paint_state(&range);
        let scenes = range.start.scene_index..range.end.scene_index;
        let scale = self.scale_factor();
        let node = |id: ScrollNodeId| plan.scaled(id, scale);
        // Groups come from the rendered frame (`plan_scrolled_replay` refuses a layer's).
        let labels = self.rendered_frame.scene.group_labels(scenes.clone(), record.groups());
        let groups = self.replay_groups(record.groups(), Some((&labels, &node)));
        self.replay_scenes_scrolled(scenes, plan);
        self.next_frame.scene.relabel = 0;
        self.allow_replayed(record);
        RetainedPaint::of(start..self.paint_index(), true, record.allowed).grouped(groups)
    }

    /// The scene commands `scenes` of the record's scene replayed with the
    /// plan's shifts, with the copied nodes' ranges and dependents in them.
    pub(super) fn replay_scenes_scrolled(&mut self, scenes: Range<usize>, plan: &ScrollReplay) {
        let scale = self.scale_factor();
        // The record's scene: the source layer's while one records
        // (`plan_scrolled_replay` with a layer), else the rendered frame's.
        let layer = self.replay_source();
        let space = layer.as_deref().map_or(self.rendered_frame.scene.scroll.space, |layer| layer.scene().scroll.space);
        let here = self.next_frame.scene.scroll.space;
        // The copied nodes' scene ranges, mapped through the replay.
        let landing = std::mem::take(&mut self.next_frame.scroll_paint.landing);
        let (mine, others): (Vec<_>, Vec<_>) = landing
            .into_iter()
            .partition(|(_, paint, of, _)| *of == space && paint.start >= scenes.start && paint.end <= scenes.end);
        self.next_frame.scroll_paint.landing = others;
        let mut edges: Vec<(usize, usize)> = mine.iter().flat_map(|(_, paint, _, _)| [(paint.start, 0), (paint.end, 0)]).collect();
        let dependents: Vec<(ScrollNodeId, usize)> = self
            .rendered_frame
            .scroll_dependents
            .iter()
            .filter(|&&(_, at, of)| of == space && scenes.contains(&at))
            .map(|&(id, at, _)| (id, at))
            .collect();
        edges.extend(dependents.iter().map(|&(_, at)| (at, 0)));
        edges.sort_unstable();
        edges.dedup();
        let node = |id: ScrollNodeId| plan.scaled(id, scale);
        let source = layer.as_deref().map_or(&self.rendered_frame.scene, |layer| layer.scene());
        let from = self.next_frame.scene.paint_operations.len();
        self.next_frame.scene.replay_scrolled(scenes, source, node, &mut edges);
        let land = |old: usize| edges[edges.partition_point(|edge| edge.0 < old)].1;
        let landed: Vec<_> = dependents.iter().map(|&(id, at)| (id, land(at), here)).collect();
        self.next_frame.scroll_dependents.extend(landed);
        for (index, paint, _, main) in mine {
            self.next_frame.scroll_nodes[index as usize].land(land(paint.start)..land(paint.end), here, main);
        }
        self.land_in_drawn_layers(from);
    }

    /// Ranges and dependents in the scene of a retained layer the copy from
    /// command `from` on draws stay where they are: the layer holds them as
    /// they were (`scroll_still`), as `land_scroll_paint` keeps them.
    fn land_in_drawn_layers(&mut self, from: usize) {
        let frame = &mut self.next_frame;
        let drawn = super::scroll_nodes::layer_spaces(&frame.scene.paint_operations[from..]);
        if drawn.is_empty() {
            return;
        }
        let landing = std::mem::take(&mut frame.scroll_paint.landing);
        let (held, rest): (Vec<_>, Vec<_>) = landing.into_iter().partition(|entry| drawn.contains(&entry.2));
        frame.scroll_paint.landing = rest;
        for (index, paint, space, main) in held {
            frame.scroll_nodes[index as usize].land(paint, space, main);
        }
        let kept: Vec<_> = self.rendered_frame.scroll_dependents.iter().filter(|entry| drawn.contains(&entry.2)).copied().collect();
        self.next_frame.scroll_dependents.extend(kept);
    }
}

#[cfg(test)]
mod kinds_tests;
#[cfg(test)]
mod tests;
