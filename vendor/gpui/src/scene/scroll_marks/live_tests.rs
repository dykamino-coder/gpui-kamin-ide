//! A rebuilt projection can move ink outside its original reservoir tiles.

use super::*;
use crate::{Pixels, TransformBuild, size};
use std::rc::Rc;

#[derive(Debug)]
struct Projection(f32);

impl TransformBuild for Projection {
    fn matrix(&self) -> TransformationMatrix {
        TransformationMatrix { rotation_scale: [[2., 0.], [0., 2.]], translation: [0., self.0] }
    }

    fn moved(&self, delta: Point<Pixels>) -> Rc<dyn TransformBuild> {
        Rc::new(Self(self.0 + delta.y.0))
    }
}

fn rect(y: f32, height: f32) -> Bounds<ScaledPixels> {
    Bounds::new(point(ScaledPixels(0.), ScaledPixels(y)), size(ScaledPixels(20.), ScaledPixels(height)))
}

fn item() -> Item {
    let pivot = Rc::new(TransformPivot::built_by(Rc::new(Projection(0.)), 1., None));
    Item {
        piece: Piece::Primitive(Primitive::Quad(Quad {
            bounds: rect(0., 10.),
            transformation: pivot.matrix(),
            content_mask: ContentMask { bounds: rect(0., 5000.) },
            ..Default::default()
        })),
        own: rect(0., 5000.),
        placed: rect(0., 20.),
        end: 0,
        pivot: Some(pivot),
    }
}

#[test]
fn selection_and_culling_use_the_rebuilt_projection() {
    let run = Run::new(ScrollNodeId(1, 0), vec![item()]);
    let shift = point(ScaledPixels(0.), ScaledPixels(1000.));
    let mut out = Vec::new();
    run.draw(shift, 0, rect(3000., 20.), &mut out);
    assert_eq!(out.len(), 1, "ink enters a clip outside its original tile");
    let PaintOperation::Primitive(p) = &out[0] else { panic!("primitive") };
    assert_eq!(placed_bounds(p).origin.y, ScaledPixels(3000.));
    out.clear();
    run.draw(shift, 0, rect(1000., 20.), &mut out);
    assert!(out.is_empty(), "translated old bounds cannot keep projected ink visible");
}

#[test]
fn nested_layer_bounds_follow_current_visible_ink() {
    let layer = |end| Item { piece: Piece::Layer(0), own: rect(0., 5000.), placed: rect(0., 20.), end, pivot: None };
    let end = || Item { piece: Piece::End, own: Bounds::default(), placed: Bounds::default(), end: 0, pivot: None };
    let run = Run::new(ScrollNodeId(1, 0), vec![layer(5), layer(4), item(), end(), end()]);
    let mut out = Vec::new();
    run.draw(point(ScaledPixels(0.), ScaledPixels(1000.)), 0, rect(3000., 20.), &mut out);
    assert_eq!(out.len(), 5);
    for operation in &out[..2] {
        let PaintOperation::StartLayer(bounds, _) = operation else { panic!("layer") };
        assert_eq!(*bounds, rect(3000., 20.));
    }
}
