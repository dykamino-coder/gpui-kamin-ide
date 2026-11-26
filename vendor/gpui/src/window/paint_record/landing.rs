//! KaminIDE patch: a paint record lands with the scroll nodes it painted.
//!
//! A node opened while a record records has its ranges in the record's
//! scene (`Window::begin_scroll_node_paint`), a scene the frame never draws.
//! When the record lands where it was recorded (no offset), its copy keeps
//! those nodes' marks (`scene::scroll_marks::carry`) and the nodes take
//! their ranges in the scene it lands in, so a scrolled replay of the frame
//! moves them as the nodes the frame painted itself. Landed elsewhere, or
//! with a command of their ranges dropped by the copy, they stay in the
//! record's scene (torn): a scrolled replay then builds, as before.

use super::*;
use crate::scene::scroll_marks::{SceneSpace, carry::Carry};

impl Window {
    /// `replay_placed_carrying` of the record `scene`, landing the nodes it
    /// painted when it lands where it was recorded.
    pub(super) fn replay_placed_landing(
        &mut self,
        scene: &Scene,
        offset: Point<ScaledPixels>,
        clip: Option<Bounds<ScaledPixels>>,
    ) -> bool {
        let from = scene.scroll.space;
        let painted = self.next_frame.scroll_nodes.iter().any(|node| node.painted_in == Some(from));
        let carry = painted && offset == Point::default();
        if carry {
            self.next_frame.scene.scroll.carry = Some(Carry::of(scene));
        }
        let done = self.replay_placed_carrying(scene, offset, clip);
        let Some(carry) = self.next_frame.scene.scroll.carry.take() else { return done };
        if done && carry.landed.len() == scene.paint_operations.len() {
            self.land_carried_nodes(from, &carry);
        }
        done
    }

    /// Move the ranges of the nodes painted in scene `from` to where
    /// `carry` says its commands landed in the scene painted now.
    fn land_carried_nodes(&mut self, from: SceneSpace, carry: &Carry) {
        let frame = &mut self.next_frame;
        let here = frame.scene.scroll.space;
        for node in frame.scroll_nodes.iter_mut().filter(|node| node.painted_in == Some(from)) {
            let paint = node.paint.as_ref().map(|range| carry.range(range));
            let more: Option<Vec<_>> = node.more.iter().map(|range| carry.range(range)).collect();
            match (paint, more) {
                (Some(Some(paint)), Some(more)) => {
                    (node.paint, node.more, node.painted_in) = (Some(paint), more, Some(here));
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
