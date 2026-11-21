//! KaminIDE patch: a group whose whole content is one box, painted again.
//!
//! A box that alone fills its composition group (a filtered, blended or
//! transformed leaf: its decorations painted by its own child elements) is
//! no run to splice: sprites, paths and decorations of child elements take
//! orders no plan of one run predicts. Its group is painted again instead:
//! the box's paint into a scene of its own is what a fresh paint of the
//! group holds, ordered from scratch, and replaces the group's scene whole.
//! The group's label and bounds stay, so the new ink must stay where the
//! group or the old content reached.

use super::SceneLayer;
use crate::scene::{Bounds, PaintOperation, ScaledPixels, Scene};

/// Whether `scene` holds a group of its own (a label of a nested group).
fn nests(scene: &Scene) -> bool {
    !scene.groups.is_empty() || scene.surfaces.iter().any(|s| s.group != 0)
}

/// The union of the clipped placed bounds of `scene`'s primitives.
fn ink(scene: &Scene) -> Option<Bounds<ScaledPixels>> {
    scene
        .paint_operations
        .iter()
        .filter_map(|op| match op {
            PaintOperation::Primitive(p) => Some(super::boxes::ink_of(p)),
            _ => None,
        })
        .filter(|b| !b.is_empty())
        .reduce(|a, b| a.union(&b))
}

impl SceneLayer {
    /// The group run `at` fills alone: its group, when no other run is in it,
    /// the run spans the group's commands and the group nests none.
    pub(crate) fn group_root(&self, at: usize) -> Option<u32> {
        let run = self.boxes.get(at)?;
        let g = run.group?;
        let scene = &self.scene.groups.get(g as usize)?.scene;
        let alone = self.boxes.iter().enumerate().all(|(i, b)| i == at || b.group != Some(g));
        let whole = run.ops.start == 0 && run.ops.end as usize == scene.paint_operations.len();
        (alone && whole && !nests(scene)).then_some(g)
    }

    /// Replace the scene of the group run `at` fills alone (`group_root`)
    /// with `new`, the box painted again into a scratch scene. `None`, the
    /// layer unchanged, when the run is no group root, `new` nests a group
    /// or inks beyond the group and the old content. Otherwise the damage:
    /// the group's composite area.
    pub(crate) fn replace_group_content(&mut self, at: usize, mut new: Scene) -> Option<Option<Bounds<ScaledPixels>>> {
        let g = self.group_root(at)?;
        if nests(&new) {
            return None;
        }
        let run = &self.boxes[at];
        let reach = self.scene.groups[g as usize].bounds.union(&run.ink.unwrap_or(run.area));
        let ink = ink(&new);
        if ink.is_some_and(|ink| reach.intersect(&ink) != ink) {
            return None;
        }
        new.in_group = true;
        new.finish();
        let run = &mut self.boxes[at];
        run.ops = 0..new.paint_operations.len() as u32;
        run.ink = ink;
        self.scene.groups[g as usize].scene = std::rc::Rc::new(new);
        self.version += 1;
        Some(self.composite_area(g).or(self.bounds))
    }
}

#[cfg(test)]
mod tests {
    use super::super::group_runs_tests::{LABEL, grouped, paint_box, quads, rect};
    use super::super::{BoxRange, SceneLayer};
    use crate::scene::{PaintGroup, PaintSurface};
    use crate::{ContentMask, Hsla, Scene, hsla};
    use std::rc::Rc;

    /// Box 1 alone in group 0 (`first` and a child's quad), its label.
    fn root(first: Hsla, child: Hsla) -> SceneLayer {
        let mut inner = Scene { in_group: true, ..Scene::default() };
        inner.book.group_runs = Some(Vec::new());
        let run = paint_root(&mut inner, first, child);
        inner.finish();
        let mut scene = Scene::default();
        scene.groups.push(PaintGroup { scene: Rc::new(inner), bounds: LABEL, ..Default::default() });
        scene.insert_group_surface(
            PaintSurface {
                order: 0,
                bounds: LABEL,
                content_mask: ContentMask { bounds: rect(0., 0., 1000., 1000.) },
                corner_radii: Default::default(),
                blur_radius: 0.,
                group: 1,
                opacity: 0.5,
                color_matrix: None,
                fragment: 0,
            },
            false,
        );
        SceneLayer::finish(scene, Vec::new(), vec![BoxRange { group: Some(0), ..run }])
    }

    /// The box's run: its fill, then a decoration a child element paints.
    fn paint_root(scene: &mut Scene, fill: Hsla, child: Hsla) -> BoxRange {
        let run = paint_box(scene, 1, rect(0., 0., 100., 40.), fill);
        scene.insert_primitive(super::super::group_runs_tests::quad(rect(10., 10., 20., 20.), child));
        BoxRange { ops: run.ops.start..scene.paint_operations.len() as u32, ..run }
    }

    const RED: Hsla = Hsla { h: 0., s: 1., l: 0.5, a: 1. };
    const BLUE: Hsla = Hsla { h: 0.66, s: 1., l: 0.5, a: 1. };

    #[test]
    fn a_group_root_painted_again_equals_a_fresh_group() {
        let mut layer = root(RED, RED);
        let fresh = root(BLUE, hsla(0.3, 1., 0.5, 1.));
        let held = Rc::clone(&layer.scene.groups[0].scene);
        assert_eq!(layer.group_root(0), Some(0));
        let mut new = Scene::default();
        paint_root(&mut new, BLUE, hsla(0.3, 1., 0.5, 1.));
        let damage = layer.replace_group_content(0, new).expect("the group painted again");
        assert_eq!(damage, Some(LABEL));
        assert_eq!(quads(&layer.scene.groups[0].scene), quads(&fresh.scene.groups[0].scene));
        assert!(layer.scene.groups[0].scene.in_group);
        assert_eq!(layer.boxes[0].ops, fresh.boxes[0].ops);
        assert_eq!(layer.version(), 1);
        // A frame that still holds the old group keeps what it drew.
        assert_eq!(quads(&held), quads(&root(RED, RED).scene.groups[0].scene));
    }

    #[test]
    fn a_run_that_shares_its_group_is_no_group_root() {
        let mut layer = grouped(RED);
        assert_eq!(layer.group_root(0), None);
        let mut new = Scene::default();
        paint_box(&mut new, 1, layer.boxes[0].area, BLUE);
        assert!(layer.replace_group_content(0, new).is_none());
        assert_eq!(layer.version(), 0);
    }

    #[test]
    fn new_ink_beyond_the_group_and_the_old_content_is_refused() {
        let mut layer = root(RED, RED);
        let before = quads(&layer.scene.groups[0].scene);
        let mut new = Scene::default();
        paint_box(&mut new, 1, rect(0., 0., 300., 40.), BLUE);
        assert!(layer.replace_group_content(0, new).is_none());
        assert_eq!(quads(&layer.scene.groups[0].scene), before);
        assert_eq!(layer.version(), 0);
    }

    #[test]
    fn new_content_that_nests_a_group_is_refused() {
        let mut layer = root(RED, RED);
        let mut new = Scene::default();
        paint_root(&mut new, BLUE, BLUE);
        new.groups.push(PaintGroup::default());
        assert!(layer.replace_group_content(0, new).is_none());
        assert_eq!(layer.version(), 0);
    }
}