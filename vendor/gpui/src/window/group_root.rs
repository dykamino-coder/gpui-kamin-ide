//! KaminIDE patch: a box that is the whole content of its group, painted
//! again with the group (`SceneLayer::replace_group_content`).
//!
//! The host repaints such a box with its child elements (its decorations
//! paint there) into a scratch scene in the paint state its recording
//! saved; the group's scene becomes that scene, ordered as a paint of the
//! group from scratch. The group's label, bounds and the layer around it
//! stay, and so does every frame that still holds the old group.

use super::*;
use crate::scene::{BoxEnv, LayerHandle};

impl Window {
    /// Paint box `owner`, the only content of its group in `layer`
    /// (`SceneLayer::group_root`), again with `paint` (given the border box
    /// and, when its recording knew it, the unrounded border box) and
    /// replace the group's scene with it. `None`, the layer unchanged, when
    /// the box is no group root, `paint` refuses (`None`) or the new paint
    /// does not fit the group; the host paints the layer again then.
    pub fn rerecord_layer_group(
        &mut self,
        layer: &LayerHandle,
        owner: u32,
        cx: &mut App,
        mut paint: impl FnMut(&mut Self, &mut App, Bounds<Pixels>, Option<Bounds<Pixels>>) -> Option<()>,
    ) -> Option<()> {
        if self.layer_recording.is_some() {
            return None;
        }
        let (at, area, env, placed) = {
            let content = layer.0.content.borrow();
            let mut runs = content.runs_of(owner);
            let at = runs.next()?;
            if runs.next().is_some() || content.group_root(at).is_none() {
                return None;
            }
            let b = &content.boxes[at];
            (at, b.area, b.env, b.placed)
        };
        let mut painted = None;
        let mut paint = |w: &mut Self, cx: &mut App, env: &BoxEnv| painted = paint(w, cx, env.bounds, env.exact);
        let (scene, bindings) = self.paint_box_run(owner, area, env, cx, &mut paint);
        painted?;
        // A run placed from a paint record: the new commands are placed the same way.
        let scene = match placed {
            Some(placed) => crate::scene::place_run(scene, bindings, placed)?.0,
            None => scene,
        };
        let damage = {
            let mut current = layer.0.content.borrow_mut();
            // The layer is held by no frame (a grouped layer is copied into frames).
            Rc::make_mut(&mut *current).replace_group_content(at, scene)?
        };
        if let Some(rect) = damage {
            let old = layer.0.damage.get();
            layer.0.damage.set(Some(old.map_or(rect, |d| d.union(&rect))));
        }
        crate::scene::layer_counters::BOXES_RERECORDED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(())
    }
}