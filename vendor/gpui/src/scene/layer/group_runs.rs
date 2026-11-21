//! KaminIDE patch: box runs inside the composition groups a layer kept
//! (`window::grouped_layer`).
//!
//! A run painted in a group's scene (`BoxRange::group`) is replaced in that
//! scene exactly as a run of the layer's own scene is replaced in it: the
//! group's scene and runs stand in for the layer's while `check_box` and
//! `replace_box` plan and splice, so the orders are a paint from scratch's
//! inside the group. The group's image changes as a whole where it is
//! composited: the damage is the area of the label of its outermost group,
//! which covers blur and blend neighbours drawn with it.

use super::{Binding, BoxRange, SceneLayer};
use crate::scene::{Bounds, ScaledPixels, Scene};

impl SceneLayer {

    /// Where group `g`'s image lands in the layer: the label of its
    /// outermost enclosing group, clipped by its content mask.
    pub(crate) fn composite_area(&self, mut g: u32) -> Option<Bounds<ScaledPixels>> {
        let label = |scene: &Scene, g: u32| {
            scene.surfaces.iter().find(|s| s.group == g + 1).map(|s| s.bounds.intersect(&s.content_mask.bounds))
        };
        // Groups nest finitely: each step goes to a later group of the list.
        for _ in 0..=self.scene.groups.len() {
            if let Some(area) = label(&self.scene, g) {
                return Some(area);
            }
            g = self.scene.groups.iter().position(|group| label(&group.scene, g).is_some())? as u32;
        }
        None
    }

    /// The area run `at` paints in the layer: its own for the layer's scene,
    /// its group's composite for a group's.
    pub(crate) fn run_area(&self, at: usize) -> Option<Bounds<ScaledPixels>> {
        let b = self.boxes.get(at)?;
        match b.group {
            None => Some(b.area),
            Some(g) => self.composite_area(g),
        }
    }

    /// `check_box` and `replace_box` of run `at` in its group's scene: the
    /// damage and whether commands after the run moved; `None` (the layer
    /// unchanged) when the run cannot be replaced in place.
    pub(crate) fn replace_group_run(
        &mut self,
        at: usize,
        new: Scene,
        bindings: Vec<Binding>,
    ) -> Option<(Option<Bounds<ScaledPixels>>, bool)> {
        let g = self.boxes.get(at)?.group?;
        // Quads in a group's scene take no bindings (`bind_painted_quad`).
        if !bindings.is_empty() {
            return None;
        }
        let members: Vec<usize> = (0..self.boxes.len()).filter(|&i| self.boxes[i].group == Some(g)).collect();
        let local = members.iter().position(|&i| i == at)?;
        let group = self.scene.groups.get_mut(g as usize)?;
        let bounds = group.bounds;
        let scene = Scene::unshared(std::mem::take(&mut group.scene));
        let boxes = members.iter().map(|&i| BoxRange { group: None, ..self.boxes[i].clone() }).collect();
        let mut sub = SceneLayer {
            scene,
            id: self.id,
            version: 0,
            bounds: Some(bounds),
            bindings: Vec::new(),
            boxes,
            by_owner: std::cell::OnceCell::new(),
        };
        let done = sub.check_box(local, &new).map(|plan| sub.replace_box(local, new, Vec::new(), plan));
        for (k, &i) in members.iter().enumerate() {
            self.boxes[i] = BoxRange { group: Some(g), ..sub.boxes[k].clone() };
        }
        self.scene.groups[g as usize].scene = std::rc::Rc::new(sub.scene);
        let (damage, shifted) = done?;
        self.version += 1;
        Some((damage.map(|d| self.composite_area(g).or(self.bounds).unwrap_or(d)), shifted))
    }
}
