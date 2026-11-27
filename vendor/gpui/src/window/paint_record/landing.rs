//! KaminIDE patch: a paint record lands with the scroll nodes it painted.
//!
//! A node opened while a record records has its ranges in the record's
//! scene (`Window::begin_scroll_node_paint`), a scene the frame never draws.
//! Where the record lands, moved and clipped (a column of a multicol flow),
//! its copy keeps those nodes' marks moved with it (`scene::scroll_marks::
//! carry`), and a node whose commands the copy shows takes its ranges in the
//! scene it lands in, its clips moved and clipped as the copy, so a scrolled
//! replay of the frame moves it as a node the frame painted itself. A node
//! shown by two landings, or with a command of its ranges dropped by the
//! copy, stays where it was (torn): a scrolled replay then builds.

use super::*;
use crate::scene::scroll_marks::{SceneSpace, carry::Carry};

#[cfg(test)]
mod tests;

impl Window {
    /// `replay_placed_carrying` of the record `scene`, landing the nodes it
    /// painted where it lands.
    pub(super) fn replay_placed_landing(
        &mut self,
        scene: &Scene,
        offset: Point<ScaledPixels>,
        clip: Option<Bounds<ScaledPixels>>,
    ) -> bool {
        let from = scene.scroll.space;
        // A record's marks are those of the nodes it opened (`ScrollBook::record_base`).
        let marked = !scene.scroll.marks.is_empty();
        if marked {
            self.next_frame.scene.scroll.carry = Some(Carry::of(scene, offset, clip));
        }
        let done = self.replay_placed_carrying(scene, offset, clip);
        let carry = self.next_frame.scene.scroll.carry.take();
        let carry = carry.filter(|carry| done && carry.landed.len() == scene.paint_operations.len());
        if marked && carry.is_none() && crate::dirty_sources::trace::enabled() {
            // DevTools `why`: the nodes stay in the record's scene, and the frame builds.
            let why = match done {
                false => format!("copied, not placed ({} groups)", scene.groups.len()),
                true => "placed per primitive".to_string(),
            };
            crate::dirty_sources::trace::push_with("scroll_node.record_unlanded", core::panic::Location::caller(), || why);
        }
        if let Some(carry) = carry {
            self.land_carried_nodes(from, &carry, offset, clip);
        }
        done
    }

    /// Move the ranges of the nodes painted in scene `from` that the copy
    /// shows to where `carry` says their commands landed in the scene
    /// painted now, their clips moved by `offset` and clipped by `clip`.
    fn land_carried_nodes(&mut self, from: SceneSpace, carry: &Carry, offset: Point<ScaledPixels>, clip: Option<Bounds<ScaledPixels>>) {
        let scale = self.scale_factor();
        let unscaled = |v: ScaledPixels| px(v.0 / scale);
        let shift = point(unscaled(offset.x), unscaled(offset.y));
        let clip = clip.map(|clip| {
            let origin = point(unscaled(clip.origin.x), unscaled(clip.origin.y));
            Bounds { origin, size: size(unscaled(clip.size.width), unscaled(clip.size.height)) }
        });
        let place = |bounds: Bounds<Pixels>| {
            let moved = Bounds { origin: bounds.origin + shift, size: bounds.size };
            clip.map_or(moved, |clip| moved.intersect(&clip))
        };
        let frame = &mut self.next_frame;
        let here = frame.scene.scroll.space;
        for at in 0..frame.scroll_nodes.len() {
            // Its real clip is its parent's, not moved by the copy, cut by its moved mask.
            let parent = frame.scroll_nodes[at].parent.map(|parent| frame.scroll_nodes[parent as usize].clip);
            let node = &mut frame.scroll_nodes[at];
            let shown = carry.kept.contains(&node.id);
            if node.painted_in != Some(from) {
                // Its commands landed already elsewhere: two landings cannot move as one node.
                node.torn |= shown && node.painted_in.is_some();
                continue;
            }
            // A node the copy hides lands where another copy of the record shows it.
            if !shown && node.ranges().any(|range| !range.is_empty()) {
                continue;
            }
            let paint = node.paint.as_ref().map(|range| carry.range(range));
            let more: Option<Vec<_>> = node.more.iter().map(|range| carry.range(range)).collect();
            match (paint, more) {
                (Some(Some(paint)), Some(more)) => {
                    (node.paint, node.more, node.painted_in) = (Some(paint), more, Some(here));
                    node.chain = place(node.chain);
                    node.clip = parent.map_or_else(|| place(node.clip), |parent| parent.intersect(&node.chain));
                    node.interest.origin += shift;
                    node.children_offset += shift;
                }
                _ => node.torn = true,
            }
        }
        // A dependent lands before the next command the copy kept.
        for dependent in frame.scroll_dependents.iter_mut().filter(|dependent| dependent.2 == from) {
            if let Some(at) = carry.range(&(dependent.1..dependent.1)) {
                (dependent.1, dependent.2) = (at.start, here);
            }
        }
    }
}