//! Construct renderable path scenes without serializing and reparsing SVG XML.

use super::*;

/// A solid path paint, shared by fills and strokes.
#[derive(Clone, Copy, Debug)]
pub struct PathPaint {
    /// Straight sRGB color.
    pub color: Color,
    /// Paint opacity.
    pub opacity: Opacity,
}

/// Stroke geometry and paint.
#[derive(Clone, Copy, Debug)]
pub struct PathStroke {
    /// Stroke paint.
    pub paint: PathPaint,
    /// Positive stroke width.
    pub width: StrokeWidth,
    /// Miter limit; joins are miter and caps are butt.
    pub miter_limit: StrokeMiterlimit,
}

/// Geometry and paints for one path.
#[derive(Clone, Debug)]
pub struct PathShape {
    /// Already resolved tiny-skia geometry.
    pub data: Arc<tiny_skia_path::Path>,
    /// Optional fill.
    pub fill: Option<PathPaint>,
    /// Fill winding rule.
    pub rule: FillRule,
    /// Optional stroke.
    pub stroke: Option<PathStroke>,
}

/// An ordered path-scene child.
#[derive(Clone, Debug)]
pub enum PathElement {
    /// One path, painted fill then stroke.
    Shape(PathShape),
    /// A transformed or composited group.
    Group(PathGroup),
}

/// Ordered paths with optional luminance masking and linear-RGB blur.
#[derive(Clone, Debug, Default)]
pub struct PathGroup {
    /// Relative transform.
    pub transform: Transform,
    /// Children in paint order.
    pub children: Vec<PathElement>,
    /// Luminance mask in scene coordinates, clipped to the scene viewport.
    pub mask: Option<Box<PathGroup>>,
    /// Optional compound clipping path in local user coordinates.
    pub clip: Option<PathShape>,
    /// Gaussian standard deviation; zero disables filtering.
    pub blur: f32,
}

impl Tree {
    /// Construct a path-only tree using the same bounds and paint machinery
    /// as parsed SVG. Mask and filter regions cover the complete viewport.
    pub fn from_paths(size: Size, root: PathGroup) -> Option<Self> {
        let mut tree = Tree {
            size,
            root: Group::empty(),
            linear_gradients: Vec::new(),
            radial_gradients: Vec::new(),
            patterns: Vec::new(),
            clip_paths: Vec::new(),
            masks: Vec::new(),
            filters: Vec::new(),
            #[cfg(feature = "text")]
            fontdb: Arc::new(fontdb::Database::new()),
        };
        tree.root = container(root, &mut tree)?;
        Some(tree)
    }
}

// Tree and resource roots are containers: resvg renders only their children.
// Keep the supplied transform, mask and filter on a real child group.
fn container(source: PathGroup, tree: &mut Tree) -> Option<Group> {
    let mut root = Group::empty();
    root.children.push(Node::Group(Box::new(build_group(
        source, Transform::default(), tree,
    )?)));
    root.calculate_bounding_boxes();
    Some(root)
}

fn build_group(source: PathGroup, parent: Transform, tree: &mut Tree) -> Option<Group> {
    let mut group = Group::empty();
    group.transform = source.transform;
    group.abs_transform = parent.pre_concat(source.transform);
    for child in source.children {
        match child {
            PathElement::Group(child) => {
                group.children.push(Node::Group(Box::new(build_group(
                    child, group.abs_transform, tree,
                )?)));
            }
            PathElement::Shape(shape) => {
                let fill = shape.fill.map(|paint| Fill {
                    paint: Paint::Color(paint.color),
                    opacity: paint.opacity,
                    rule: shape.rule,
                    context_element: None,
                });
                let stroke = shape.stroke.map(|stroke| Stroke {
                    paint: Paint::Color(stroke.paint.color),
                    opacity: stroke.paint.opacity,
                    width: stroke.width,
                    miterlimit: stroke.miter_limit,
                    dasharray: None,
                    dashoffset: 0.0,
                    linecap: LineCap::Butt,
                    linejoin: LineJoin::Miter,
                    context_element: None,
                });
                if let Some(path) = Path::new(
                    String::new(), true, fill, stroke, PaintOrder::default(),
                    ShapeRendering::default(), shape.data, group.abs_transform,
                ) {
                    group.children.push(Node::Path(Box::new(path)));
                }
            }
        }
    }
    let rect = NonZeroRect::from_xywh(0.0, 0.0, tree.size.width(), tree.size.height())?;
    if let Some(mask) = source.mask {
        let root = container(*mask, tree)?;
        let mask = Arc::new(Mask {
            id: NonEmptyString::new(format!("path-mask-{}", tree.masks.len()))?,
            rect,
            kind: MaskType::Luminance,
            mask: None,
            root,
        });
        group.mask = Some(mask.clone());
        tree.masks.push(mask);
    }
    if let Some(shape) = source.clip {
        let root = build_group(PathGroup { children: vec![PathElement::Shape(shape)], ..Default::default() }, Transform::default(), tree)?;
        let clip = Arc::new(ClipPath {
            id: NonEmptyString::new(format!("path-clip-{}", tree.clip_paths.len()))?,
            transform: Transform::default(), clip_path: None, root,
        });
        group.clip_path = Some(clip.clone());
        tree.clip_paths.push(clip);
    }
    if source.blur > 0.0 {
        let sigma = PositiveF32::new(source.blur)?;
        let filter = Arc::new(filter::Filter {
            id: NonEmptyString::new(format!("path-blur-{}", tree.filters.len()))?,
            rect,
            primitives: vec![filter::Primitive {
                rect,
                color_interpolation: filter::ColorInterpolation::LinearRGB,
                result: String::new(),
                kind: filter::Kind::GaussianBlur(filter::GaussianBlur {
                    input: filter::Input::SourceGraphic,
                    std_dev_x: sigma,
                    std_dev_y: sigma,
                }),
            }],
        });
        group.filters.push(filter.clone());
        tree.filters.push(filter);
    }
    // Empty groups retain the same harmless dummy bounds as the parser.
    group.calculate_bounding_boxes();
    Some(group)
}

#[cfg(test)]
mod tests;
