//! Path-scene resource and coordinate contracts, independent of XML parsing.
use super::*;

fn rect() -> PathShape {
    let data = tiny_skia_path::PathBuilder::from_rect(
        Rect::from_xywh(1.5, 2.5, 10.0, 12.0).unwrap(),
    );
    PathShape {
        data: Arc::new(data),
        fill: Some(PathPaint { color: Color::white(), opacity: Opacity::ONE }),
        rule: FillRule::NonZero,
        stroke: None,
    }
}

#[test]
fn paths_keep_absolute_bounds_through_nested_transforms() {
    let child = PathGroup {
        transform: Transform::from_scale(2.0, 3.0),
        children: vec![PathElement::Shape(rect())],
        ..Default::default()
    };
    let tree = Tree::from_paths(Size::from_wh(80.0, 60.0).unwrap(), PathGroup {
        transform: Transform::from_translate(4.0, 5.0),
        children: vec![PathElement::Group(child)],
        ..Default::default()
    }).unwrap();
    let Node::Group(root) = &tree.root().children()[0] else { panic!("root group") };
    let Node::Group(child) = &root.children()[0] else { panic!("group") };
    let Node::Path(path) = &child.children()[0] else { panic!("path") };
    assert_eq!(path.abs_bounding_box(), Rect::from_xywh(7.0, 12.5, 20.0, 36.0).unwrap());
}

#[test]
fn mask_clip_and_blur_are_registered_with_viewport_regions() {
    let mask = PathGroup {
        children: vec![PathElement::Shape(rect())],
        ..Default::default()
    };
    let tree = Tree::from_paths(Size::from_wh(80.0, 60.0).unwrap(), PathGroup {
        children: vec![PathElement::Shape(rect())],
        mask: Some(Box::new(mask)),
        clip: Some(rect()),
        blur: 2.0,
        ..Default::default()
    }).unwrap();
    assert_eq!(tree.masks.len(), 1);
    assert_eq!(tree.clip_paths.len(), 1);
    assert_eq!(tree.filters.len(), 1);
    let viewport = NonZeroRect::from_xywh(0.0, 0.0, 80.0, 60.0).unwrap();
    let Node::Group(root) = &tree.root().children()[0] else { panic!("root group") };
    assert_eq!(root.mask().unwrap().rect(), viewport);
    assert_eq!(root.filters()[0].rect(), viewport);
}
