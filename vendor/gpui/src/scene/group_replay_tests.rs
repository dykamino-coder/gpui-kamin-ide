//! KaminIDE patch: a replay carries the groups of its range and renumbers labels.

use super::*;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds { origin: point(ScaledPixels(x), ScaledPixels(y)), size: Size { width: ScaledPixels(w), height: ScaledPixels(h) } }
}

fn quad(x: f32, y: f32) -> Quad {
    let bounds = rect(x, y, 10., 10.);
    Quad { bounds, content_mask: ContentMask { bounds }, transformation: TransformationMatrix::unit(), ..Default::default() }
}

fn label(group: u32, x: f32) -> PaintSurface {
    let bounds = rect(x, 0., 10., 10.);
    PaintSurface {
        order: 0,
        bounds,
        content_mask: ContentMask { bounds },
        corner_radii: Default::default(),
        blur_radius: 0.,
        group,
        opacity: 0.5,
        color_matrix: None,
        fragment: 0,
    }
}

fn group(scene: Scene, x: f32) -> PaintGroup {
    PaintGroup { scene: std::rc::Rc::new(scene), bounds: rect(x, 0., 10., 10.), mask_bounds: rect(x, 0., 10., 10.), ..Default::default() }
}

fn finished(mut scene: Scene) -> Scene {
    scene.finish();
    scene
}

/// A frame whose range (from command 1) made group 1 (nested, labelled in
/// group 2's scene) and group 2 (labelled in the frame), after one earlier
/// group of another owner.
fn frame() -> Scene {
    let mut frame = Scene::default();
    frame.groups.push(group(Scene::default(), 500.));
    frame.insert_primitive(quad(500., 0.));
    let mut nested = Scene { in_group: true, ..Scene::default() };
    nested.insert_primitive(quad(20., 0.));
    frame.groups.push(group(finished(nested), 20.));
    let mut outer = Scene { in_group: true, ..Scene::default() };
    outer.insert_primitive(quad(0., 0.));
    outer.insert_primitive(label(2, 20.));
    frame.groups.push(group(finished(outer), 0.));
    frame.insert_primitive(label(3, 0.));
    frame.insert_primitive(quad(40., 0.));
    finished(frame)
}

fn labels(scene: &Scene) -> Vec<u32> {
    scene.surfaces.iter().map(|surface| surface.group).collect()
}

/// Replay commands `1..` and groups `1..3` of `prev` the way
/// `Window::replay_groups` does, into a frame that made `before` groups.
fn replay(prev: &Scene, before: usize) -> Scene {
    let mut next = Scene::default();
    for _ in 0..before {
        next.groups.push(group(Scene::default(), 900.));
    }
    let delta = before as i32 - 1;
    let still = |_| (Point::default(), Bounds::default());
    let copies: Vec<_> = prev.groups[1..3].iter().map(|g| Scene::replayed_group(g, delta, None, &still)).collect();
    next.groups.extend(copies);
    next.relabel = delta;
    next.replay(1..prev.paint_operations.len(), prev);
    next.relabel = 0;
    finished(next)
}

#[test]
fn a_replay_lands_its_groups_after_the_frames_and_renumbers_every_label() {
    let prev = frame();
    for before in [0, 1, 3] {
        let next = replay(&prev, before);
        assert_eq!(next.groups.len(), before + 2);
        // The frame's label names the outer copy; the outer copy's label the nested one.
        assert_eq!(labels(&next), vec![before as u32 + 2]);
        let outer = &next.groups[before + 1];
        assert_eq!(labels(&outer.scene), vec![before as u32 + 1]);
        assert_eq!(outer.scene.quads.len(), 1);
        assert_eq!(next.groups[before].scene.quads[0].bounds, rect(20., 0., 10., 10.));
        assert_eq!(outer.bounds, prev.groups[2].bounds);
    }
}

#[test]
fn a_replay_at_the_recorded_place_copies_labels_unchanged() {
    let prev = frame();
    let next = replay(&prev, 1);
    assert_eq!(labels(&next), labels(&prev));
    assert_eq!(next.quads.len(), 1);
}

#[test]
fn renumbering_reaches_dormant_labels_without_touching_the_shared_command() {
    let shared = std::rc::Rc::new(Primitive::Surface(label(3, 0.)));
    let scene = Scene { relabel: -2, ..Scene::default() };
    let copied = scene.relabeled(&PaintOperation::Dormant(shared.clone()));
    let PaintOperation::Dormant(copy) = copied else { panic!("not dormant") };
    assert!(matches!(&*copy, Primitive::Surface(s) if s.group == 1));
    assert!(matches!(&*shared, Primitive::Surface(s) if s.group == 3));
    // A command that is no label is the same command.
    let plain = scene.relabeled(&PaintOperation::Primitive(Primitive::Quad(quad(0., 0.))));
    assert!(matches!(plain, PaintOperation::Primitive(Primitive::Quad(_))));
}

/// Groups 1 (nested, labelled in group 2's scene) and 2 (labelled in the
/// frame with mask `mask`, under the node when `marked`), each holding a
/// command under a scroll node.
fn labelled(mask: Bounds<ScaledPixels>, marked: bool) -> Scene {
    let (node, chain) = (crate::ScrollNodeId(7), rect(0., 0., 1000., 1000.));
    let mut frame = Scene::default();
    let mut nested = Scene { in_group: true, ..Scene::default() };
    nested.scroll.open = Some((node, chain));
    nested.insert_primitive(quad(20., 0.));
    nested.scroll.open = None;
    frame.groups.push(group(finished(nested), 20.));
    let mut outer = Scene { in_group: true, ..Scene::default() };
    outer.scroll.open = Some((node, chain));
    outer.insert_primitive(quad(0., 0.));
    outer.scroll.open = None;
    outer.insert_primitive(label(1, 20.));
    frame.groups.push(group(finished(outer), 0.));
    frame.scroll.open = marked.then_some((node, chain));
    frame.insert_group_surface(PaintSurface { content_mask: ContentMask { bounds: mask }, ..label(2, 0.) }, false);
    frame.scroll.open = None;
    frame.insert_primitive(quad(40., 0.));
    finished(frame)
}

#[test]
fn a_group_whose_label_its_node_dropped_is_hidden_and_copied_by_any_replay() {
    // Outside its own mask under its node, the label shows at no offset: the
    // group it labels is composited by no replay, nor is the group nested in it.
    let frame = labelled(rect(300., 300., 10., 10.), true);
    assert!(labels(&frame).is_empty());
    let found = frame.group_labels(0..frame.paint_operations.len(), 0..2);
    assert_eq!(found, vec![GroupLabel::Hidden; 2]);
    assert!(frame.groups_scrollable(0..2, &found, |_| false));
    // Dropped under no node (a record's, marked where its copy lands), a
    // later offset may show it: its commands under a node must not be copied.
    let frame = labelled(rect(300., 300., 10., 10.), false);
    let found = frame.group_labels(0..frame.paint_operations.len(), 0..2);
    assert_eq!(found, vec![GroupLabel::Unmarked; 2]);
    assert!(!frame.groups_scrollable(0..2, &found, |_| true));
    // Drawn under its node, the group moves with it.
    let frame = labelled(rect(0., 0., 10., 10.), true);
    let found = frame.group_labels(0..frame.paint_operations.len(), 0..2);
    assert_eq!(found, vec![GroupLabel::Unmarked, GroupLabel::Under(crate::ScrollNodeId(7))]);
}
#[test]
fn moving_nested_groups_into_the_frame_renumbers_their_dormant_labels() {
    // A nested group's label kept dormant under a node in the enclosing
    // group's scene names the nested group after the frame's list grew,
    // as a drawn label does (`shift_group_ids`): the enclosing group then
    // finds it under its node, and a scrolled replay moves both.
    let (node, chain) = (crate::ScrollNodeId(7), rect(0., 0., 100., 100.));
    let mut nested = Scene { in_group: true, ..Scene::default() };
    nested.scroll.open = Some((node, chain));
    nested.insert_primitive(quad(200., 0.));
    nested.scroll.open = None;
    let mut outer = Scene { in_group: true, ..Scene::default() };
    outer.scroll.open = Some((node, chain));
    outer.insert_primitive(label(1, 200.));
    outer.scroll.open = None;
    assert!(matches!(outer.paint_operations[..], [PaintOperation::Dormant(_)]), "below the chain: kept dormant");
    let base = 4;
    outer.shift_group_ids(base);
    let PaintOperation::Dormant(kept) = &outer.paint_operations[0] else { panic!("not dormant") };
    assert!(matches!(&**kept, Primitive::Surface(s) if s.group == base + 1));
    let mut frame = Scene::default();
    for _ in 0..base {
        frame.groups.push(group(Scene::default(), 900.));
    }
    frame.groups.push(group(finished(nested), 200.));
    frame.groups.push(group(finished(outer), 0.));
    frame.scroll.open = Some((node, chain));
    frame.insert_group_surface(label(base + 2, 0.), false);
    frame.scroll.open = None;
    let frame = finished(frame);
    let base = base as usize;
    let found = frame.group_labels(0..frame.paint_operations.len(), base..base + 2);
    assert_eq!(found, vec![GroupLabel::Under(node); 2]);
    assert!(frame.groups_scrollable(base..base + 2, &found, |id| id == node));
}