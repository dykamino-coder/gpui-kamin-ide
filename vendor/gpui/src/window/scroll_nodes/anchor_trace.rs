//! KaminIDE patch (DevTools `why`): anchors the frame drew without paint.

use super::*;

impl Window {
    /// Anchors of the frame just drawn whose children never painted (`paint`
    /// none): prepaint opened them, paint did not pair them. A copied node
    /// tells where its range was left unlanded (`scroll_paint.landing`): in
    /// the frame's scene, a group's or another (a layer's).
    #[track_caller]
    pub(crate) fn trace_unpainted_anchors(&self) {
        if !crate::dirty_sources::trace::enabled() {
            return;
        }
        let site = core::panic::Location::caller();
        let frame = &self.rendered_frame;
        let space_of = |space: SceneSpace| {
            if space == frame.scene.scroll.space {
                return format!("the frame's scene ({} commands)", frame.scene.paint_operations.len());
            }
            match frame.scene.groups.iter().position(|group| group.scene.scroll.space == space) {
                Some(at) => format!("the scene of group {at} of {}", frame.scene.groups.len()),
                None => format!("another scene {space:?} (a layer's)"),
            }
        };
        for (at, node) in frame.scroll_nodes.iter().enumerate() {
            if node.anchored && node.paint.is_none() {
                crate::dirty_sources::trace::push_with("scroll_node.anchor_unpainted", site, || {
                    let left: Vec<_> = frame.scroll_paint.landing.iter().filter(|entry| entry.0 as usize == at).collect();
                    let landing = match left.first() {
                        None => "no range left to land".to_string(),
                        Some((_, paint, space, main)) => {
                            format!("{} ranges left unlanded, first {paint:?} (main {main}) in {}", left.len(), space_of(*space))
                        }
                    };
                    format!("node {at} id {} chain {:?} clip {:?} parent {:?}; {landing}", node.id.0, node.chain, node.clip, node.parent)
                });
            }
        }
    }
}