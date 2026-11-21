//! A layer that kept its composition groups: box runs inside a group are
//! replaced in the group's scene, and the result equals a fresh recording.

use super::{BoxRange, SceneLayer};
use crate::scene::{PaintGroup, PaintSurface};
use crate::{Background, Bounds, ContentMask, Hsla, Quad, ScaledPixels, Scene, hsla, point, size};
use std::rc::Rc;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds { origin: point(ScaledPixels(x), ScaledPixels(y)), size: size(ScaledPixels(w), ScaledPixels(h)) }
}

fn quad(b: Bounds<ScaledPixels>, color: Hsla) -> Quad {
    Quad { bounds: b, content_mask: ContentMask { bounds: rect(0., 0., 1000., 1000.) }, background: Background::from(color), ..Default::default() }
}

fn env() -> super::BoxEnv {
    super::BoxEnv {
        bounds: Bounds::default(),
        mask: ContentMask { bounds: Bounds::default() },
        transformation: crate::TransformationMatrix::unit(),
        opacity: 1.,
        rem_size: crate::px(16.),
        fill: None,
        layer: None,
        painter: crate::scene::RunPainter::Box(crate::scene::BoxContents::Own),
    }
}

fn paint_box(scene: &mut Scene, owner: u32, area: Bounds<ScaledPixels>, color: Hsla) -> BoxRange {
    assert!(scene.open_box(owner, area, env()), "a run opens in a layer's group");
    scene.insert_primitive(quad(area, color));
    scene.close_box().expect("the box was open")
}

const LABEL: Bounds<ScaledPixels> = Bounds {
    origin: crate::Point { x: ScaledPixels(0.), y: ScaledPixels(0.) },
    size: crate::Size { width: ScaledPixels(100.), height: ScaledPixels(40.) },
};

/// Boxes 1 and 2 inside group 0 (box 1 in `first`), its label, box 3 on top.
fn grouped(first: Hsla) -> SceneLayer {
    let mut inner = Scene { in_group: true, ..Scene::default() };
    inner.book.group_runs = Some(Vec::new());
    let mut runs = vec![paint_box(&mut inner, 1, rect(0., 0., 100., 20.), first)];
    runs.push(paint_box(&mut inner, 2, rect(0., 20., 100., 20.), hsla(0.3, 1., 0.5, 1.)));
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
    let mut boxes: Vec<BoxRange> = runs.into_iter().map(|run| BoxRange { group: Some(0), ..run }).collect();
    boxes.push(paint_box(&mut scene, 3, rect(0., 50., 100., 20.), hsla(0., 0., 0., 1.)));
    SceneLayer::finish(scene, Vec::new(), boxes)
}

fn quads(scene: &Scene) -> Vec<String> {
    scene.quads.iter().map(|q| format!("{q:?}")).collect()
}

#[test]
fn a_run_in_a_group_is_replaced_there_and_equals_a_fresh_recording() {
    let (red, blue) = (hsla(0., 1., 0.5, 1.), hsla(0.66, 1., 0.5, 1.));
    let mut layer = grouped(red);
    let fresh = grouped(blue);
    // A frame still holds the old group: it keeps the content it drew.
    let held = Rc::clone(&layer.scene.groups[0].scene);
    let top = quads(&layer.scene);
    let mut new = Scene::default();
    paint_box(&mut new, 1, layer.boxes[0].area, blue);
    let (damage, moved) = layer.replace_group_run(0, new, Vec::new()).expect("replaced in place");
    assert!(!moved);
    // The group's image changes where its label composites it.
    assert_eq!(damage, Some(LABEL));
    assert_eq!(quads(&layer.scene.groups[0].scene), quads(&fresh.scene.groups[0].scene));
    assert_eq!(quads(&layer.scene), top, "the layer's own commands stay");
    assert_eq!(quads(&held), quads(&grouped(red).scene.groups[0].scene));
    assert_eq!(layer.version(), 1);
    assert_eq!(layer.boxes.iter().map(|b| (b.owner, b.group, b.ops.clone())).collect::<Vec<_>>(),
        fresh.boxes.iter().map(|b| (b.owner, b.group, b.ops.clone())).collect::<Vec<_>>());
}

#[test]
fn a_group_run_whose_ink_leaves_the_group_is_refused_and_the_layer_kept() {
    let mut layer = grouped(hsla(0., 1., 0.5, 1.));
    let before = quads(&layer.scene.groups[0].scene);
    let mut new = Scene::default();
    paint_box(&mut new, 1, rect(0., 0., 300., 20.), hsla(0.66, 1., 0.5, 1.));
    assert!(layer.replace_group_run(0, new, Vec::new()).is_none());
    assert_eq!(quads(&layer.scene.groups[0].scene), before);
    assert_eq!(layer.version(), 0);
}

#[test]
fn an_unshared_group_is_written_without_a_copy() {
    let mut group = PaintGroup { scene: Rc::new(Scene::default()), ..Default::default() };
    let at = Rc::as_ptr(&group.scene);
    group.scene_mut().in_group = true;
    assert_eq!(Rc::as_ptr(&group.scene), at);
    let held = group.clone();
    group.scene_mut().in_group = false;
    assert_ne!(Rc::as_ptr(&group.scene), Rc::as_ptr(&held.scene));
    assert!(held.scene.in_group);
}
