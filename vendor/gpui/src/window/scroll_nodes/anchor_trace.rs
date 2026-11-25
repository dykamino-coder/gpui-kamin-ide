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
        for (at, node) in frame.scroll_nodes.iter().enumerate() {
            if node.anchored && node.paint.is_none() {
                crate::dirty_sources::trace::push_with("scroll_node.anchor_unpainted", site, || {
                    let left: Vec<_> = frame.scroll_paint.landing.iter().filter(|entry| entry.0 as usize == at).collect();
                    let landing = match left.first() {
                        None => "no range left to land".to_string(),
                        Some((_, paint, space, main)) => {
                            format!("{} ranges left unlanded, first {paint:?} (main {main}) in {}", left.len(), self.describe_space(*space))
                        }
                    };
                    format!("node {at} id {} chain {:?} clip {:?} parent {:?}; {landing}", node.id.0, node.chain, node.clip, node.parent)
                });
            }
        }
    }

    /// Which scene of the rendered frame `space` is, for the journal: the
    /// frame's, a group's, or a retained layer's the frame draws.
    pub(crate) fn describe_space(&self, space: SceneSpace) -> String {
        let scene = &self.rendered_frame.scene;
        if space == scene.scroll.space {
            return format!("the frame's scene ({} commands)", scene.paint_operations.len());
        }
        if let Some(at) = scene.groups.iter().position(|group| group.scene.scroll.space == space) {
            return format!("the scene of group {at} of {}", scene.groups.len());
        }
        let layer = scene.paint_operations.iter().position(|operation| layer_spaces(std::slice::from_ref(operation)) == [space]);
        match layer {
            Some(op) => format!("the scene of the retained layer at command {op} of the frame's"),
            None => "a scene the frame does not draw (a record's or a nested layer's)".into(),
        }
    }
}
