//! Размещение типизированных путей обрезки через общий композитор масок.

use super::{MaskInput, composite_mask};
use crate::paint::effects::grouped_element::Grouped;
use crate::svg::path_commands::{Command, Geometry, Sink};
use crate::svg::path_scene::{FillRule, Scene, coverage};
use gpui::{Bounds, Pixels, Point, Window};

pub(crate) fn polygon_scene(
    polygon: &[Point<Pixels>],
    bounds: Bounds<Pixels>,
    evenodd: bool,
) -> Option<Scene> {
    let mut path = Geometry::default();
    for (i, point) in polygon.iter().enumerate() {
        let v = [
            f32::from(point.x - bounds.origin.x),
            f32::from(point.y - bounds.origin.y),
        ];
        path.command(
            false,
            if i == 0 {
                Command::Move(v)
            } else {
                Command::Line(v)
            },
        );
    }
    path.command(false, Command::Close);
    Some(coverage(
        bounds.size.width.into(),
        bounds.size.height.into(),
        path.finish(),
        if evenodd {
            FillRule::EvenOdd
        } else {
            FillRule::NonZero
        },
    ))
}

pub(crate) fn resolve_scene_mask(
    scene: &Scene,
    group: &Grouped,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) -> Option<(std::sync::Arc<gpui::RenderImage>, Bounds<Pixels>, u32)> {
    let [ot, right, bottom, ol] = group.mask_origin_off;
    composite_mask(
        group,
        &[MaskInput::Scene(scene)],
        bounds,
        f32::from(bounds.size.width) - ol - right,
        f32::from(bounds.size.height) - ot - bottom,
        ol,
        ot,
        window,
        |_, _, auto| auto,
        group.values.vcx(),
    )
}
