//! KaminIDE patch: a paint record lands with the scroll nodes it painted.
//!
//! A node opened while a record records has its ranges in the record's
//! scene (`Window::begin_scroll_node_paint`), a scene the frame never draws.
//! Where the record lands, moved and clipped (a column of a multicol flow),
//! its copy keeps those nodes' marks moved with it (`scene::scroll_marks::
//! carry`). Landed in another record's scene (a plan's invocation), the copy
//! only relays the nodes: that scene keeps the landing (`ScrollBook::relayed`)
//! for its own. Landed in a scene the frame draws, a node whose commands the
//! copy shows takes its ranges there through every landing on the way, its
//! mask moved and clipped as the copy, so a scrolled replay of the frame
//! moves it as a node the frame painted itself. A node shown by two
//! landings, or with a command of its ranges dropped by a copy, stays where
//! it was (torn): a scrolled replay then builds.

use super::*;
use crate::scene::scroll_marks::carry::{Carry, Relay};

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
        if let Some(carry) = carry.map(Carry::landed) {
            let mut relays: Vec<Relay> = (scene.scroll.relayed.iter())
                .map(|(space, hops)| (*space, hops.iter().cloned().chain([carry.clone()]).collect()))
                .collect();
            relays.push((scene.scroll.space, vec![carry]));
            match self.next_frame.scene.scroll.record_base {
                Some(_) => self.next_frame.scene.scroll.relayed.extend(relays),
                None => self.land_carried_nodes(&relays),
            }
        }
        done
    }

    /// Move the ranges of the nodes painted in the scenes of `relays` that
    /// the last landing shows to where their landings put their commands in
    /// the scene painted now, their masks moved and clipped by each landing.
    fn land_carried_nodes(&mut self, relays: &[Relay]) {
        let scale = self.scale_factor();
        let unscaled = |v: ScaledPixels| px(v.0 / scale);
        let place = |bounds: Bounds<Pixels>, hop: &Carry| {
            let (offset, clip) = hop.placement();
            let shift = point(unscaled(offset.x), unscaled(offset.y));
            let moved = Bounds { origin: bounds.origin + shift, size: bounds.size };
            let clip = clip.map(|clip| {
                let origin = point(unscaled(clip.origin.x), unscaled(clip.origin.y));
                Bounds { origin, size: size(unscaled(clip.size.width), unscaled(clip.size.height)) }
            });
            (clip.map_or(moved, |clip| moved.intersect(&clip)), shift)
        };
        let Some(last) = relays.last().and_then(|(_, hops)| hops.last()) else { return };
        let frame = &mut self.next_frame;
        let here = frame.scene.scroll.space;
        for at in 0..frame.scroll_nodes.len() {
            // Its real clip is its parent's, not moved by the copy, cut by its moved mask.
            let parent = frame.scroll_nodes[at].parent.map(|parent| frame.scroll_nodes[parent as usize].clip);
            let node = &mut frame.scroll_nodes[at];
            let shown = last.kept.contains(&node.id);
            let Some((_, hops)) = relays.iter().find(|(space, _)| node.painted_in == Some(*space)) else {
                // Its commands landed already elsewhere: two landings cannot move as one node.
                if shown && node.painted_in.is_some() {
                    node.tear();
                }
                continue;
            };
            // A node the copy hides lands where another copy of the record shows it.
            if !shown && node.ranges().any(|range| !range.is_empty()) {
                continue;
            }
            let land = |range: &Range<usize>| hops.iter().try_fold(range.clone(), |range, hop| hop.range(&range));
            let paint = node.paint.as_ref().map(land);
            let more: Option<Vec<_>> = node.more.iter().map(land).collect();
            match (paint, more) {
                (Some(Some(paint)), Some(more)) => {
                    (node.paint, node.more, node.painted_in) = (Some(paint), more, Some(here));
                    for hop in hops {
                        let (chain, shift) = place(node.chain, hop);
                        node.chain = chain;
                        node.interest.origin += shift;
                        node.children_offset += shift;
                    }
                    node.clip = match parent {
                        Some(parent) => parent.intersect(&node.chain),
                        None => hops.iter().fold(node.clip, |clip, hop| place(clip, hop).0),
                    };
                }
                _ => node.tear(),
            }
        }
        // A dependent lands before the next command the copies kept.
        for dependent in frame.scroll_dependents.iter_mut() {
            let Some((_, hops)) = relays.iter().find(|(space, _)| dependent.2 == *space) else { continue };
            let at = dependent.1;
            if let Some(landed) = hops.iter().try_fold(at..at, |range, hop| hop.range(&range)) {
                (dependent.1, dependent.2) = (landed.start, here);
            }
        }
    }
}