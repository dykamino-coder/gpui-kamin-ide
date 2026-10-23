//! Типизированные сцены декораций, растеризуемые общим конвейером resvg.

mod fingerprint;
#[cfg(test)]
mod tests;
use super::path_commands::Geometry;
use crate::style::values::basic_shape::commands::{Command, Sink};
use crate::style::values::value::Color as CssColor;
use gpui::path_scene::{Color, Opacity, Size, StrokeMiterlimit, StrokeWidth, Transform, Tree};
pub(crate) use gpui::path_scene::{
    FillRule, PathElement, PathGroup, PathPaint, PathShape, PathStroke,
};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct Scene {
    pub root: PathGroup,
    pub width: f32,
    pub height: f32,
}

impl Scene {
    pub fn new(width: f32, height: f32, root: PathGroup) -> Self {
        Self {
            root,
            width,
            height,
        }
    }

    pub fn rasterize(&self) -> Option<Arc<gpui::RenderImage>> {
        let key = fingerprint::scene(self);
        super::raster::cached(key, self.width, self.height, || {
            let tree =
                Tree::from_paths(Size::from_wh(self.width, self.height)?, self.root.clone())?;
            gpui::svg_tree_to_image(&tree, self.width, self.height, super::DENSITY)
        })
    }
}

pub(crate) fn paint(c: CssColor) -> PathPaint {
    PathPaint {
        color: Color::new_rgb(
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8,
        ),
        opacity: Opacity::new_clamped(c.a),
    }
}

pub(crate) fn gray(white: bool) -> PathPaint {
    let v = if white { 255 } else { 0 };
    PathPaint {
        color: Color::new_rgb(v, v, v),
        opacity: Opacity::ONE,
    }
}

pub(crate) fn group(children: Vec<PathElement>) -> PathGroup {
    PathGroup {
        children,
        ..Default::default()
    }
}

pub(crate) fn translated(mut source: PathGroup, x: f32, y: f32) -> PathGroup {
    source.transform = Transform::from_translate(x, y).pre_concat(source.transform);
    source
}

pub(crate) fn element(source: PathGroup) -> PathElement {
    PathElement::Group(source)
}

pub(crate) fn shape(
    data: Arc<gpui::path_scene::tiny_skia_path::Path>,
    rule: FillRule,
    fill: Option<PathPaint>,
    stroke: Option<(PathPaint, f32, f32)>,
) -> PathElement {
    PathElement::Shape(PathShape {
        data,
        rule,
        fill,
        stroke: stroke.and_then(|(paint, width, limit)| {
            Some(PathStroke {
                paint,
                width: StrokeWidth::new(width)?,
                miter_limit: StrokeMiterlimit::new(limit),
            })
        }),
    })
}

pub(crate) fn rectangle(x: f32, y: f32, w: f32, h: f32, paint: PathPaint) -> Option<PathElement> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let mut d = Geometry::default();
    d.command(false, Command::Move([x, y]));
    d.command(false, Command::Line([x + w, y]));
    d.command(false, Command::Line([x + w, y + h]));
    d.command(false, Command::Line([x, y + h]));
    d.command(false, Command::Close);
    Some(shape(d.finish()?, FillRule::NonZero, Some(paint), None))
}

pub(crate) fn masked(
    width: f32,
    height: f32,
    paint: PathPaint,
    mask: PathGroup,
) -> Option<PathGroup> {
    let mut g = group(vec![rectangle(0.0, 0.0, width, height, paint)?]);
    g.mask = Some(Box::new(mask));
    Some(g)
}

/// White coverage of a resolved path, before device-grid mask composition.
pub(crate) fn coverage(
    width: f32,
    height: f32,
    data: Option<Arc<gpui::path_scene::tiny_skia_path::Path>>,
    rule: FillRule,
) -> Scene {
    Scene::new(
        width,
        height,
        group(
            data.into_iter()
                .map(|data| shape(data, rule, Some(gray(true)), None))
                .collect(),
        ),
    )
}

/// Resample coverage onto the same device grid as CSS path masks.
pub(crate) fn device_mask(scene: &Scene, scale: f32) -> Option<Arc<gpui::RenderImage>> {
    let (w, h) = (
        (scene.width * scale).round().max(1.0) as u32,
        (scene.height * scale).round().max(1.0) as u32,
    );
    crate::paint::background::compose_mask_layers(
        &[crate::paint::background::MaskLayer {
            image: scene.rasterize()?,
            tile: [0.0, 0.0, scene.width * scale, scene.height * scale],
            no_repeat: (true, true),
            gap: (0.0, 0.0),
            snap: false,
            op: 0,
            luminance: true,
        }],
        w,
        h,
    )
}
