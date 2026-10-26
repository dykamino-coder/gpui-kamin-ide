//! Плоское неизменяемое дерево команд и переоткрываемых механических scopes.

use super::*;

/// Inputs to the existing group compositor, in source coordinates.
/// The host chooses effect policy and which records share one group.
#[derive(Clone)]
pub struct PaintGroupDescriptor {
    /// Complete output bounds of the group.
    pub bounds: Bounds<Pixels>,
    /// Rounded composite corners.
    pub corner_radii: Corners<Pixels>,
    /// Blur radius in logical pixels.
    pub blur_radius: f32,
    /// Opacity applied once to the complete group.
    pub opacity: f32,
    /// Renderer blend operation, as accepted by Window::paint_group.
    pub blend: u32,
    /// Composite coverage polygon; empty means the group's rectangle.
    pub polygon: Vec<Point<Pixels>>,
    /// Invert polygon coverage.
    pub polygon_inverse: bool,
    /// Retained image, source-space tile bounds and repeat restriction bits.
    pub mask: Option<(Arc<RenderImage>, Bounds<Pixels>, u32)>,
    /// Device-space mask clip at the record's raster scale.
    pub mask_clip: Option<[f32; 4]>,
    /// Device-space projective map retained from a source ancestor plane.
    pub projection: Option<[f32; 9]>,
    /// Source-space BSP coverage pieces, in dependency order.
    pub fragments: Vec<Vec<Point<Pixels>>>,
}

impl Default for PaintGroupDescriptor {
    fn default() -> Self {
        Self {
            bounds: Bounds::default(), corner_radii: Corners::default(),
            blur_radius: 0.0, opacity: 1.0, blend: 0, polygon: Vec::new(),
            polygon_inverse: false, mask: None, mask_clip: None, projection: None, fragments: Vec::new(),
        }
    }
}

/// A mechanical source-space ancestor scope, without CSS classification.
#[derive(Clone)]
pub enum PaintScopeDescriptor {
    /// Intersect subsequent output with a source-space rectangular clip.
    Clip(Bounds<Pixels>),
    /// Composite all nested records as one group using the existing backend.
    Group(PaintGroupDescriptor),
}

/// Ordered immutable drawing plan. Construction retains no element callbacks.
/// A flat balanced stream avoids recursive ownership and recursive destruction.
#[derive(Clone, Default)]
pub struct PaintPlan(Rc<[Command]>);

#[derive(Clone)]
enum Command {
    Record(PaintRecord),
    Begin(Box<PaintScopeDescriptor>),
    End,
}

impl PaintPlan {
    /// One shared immutable record.
    pub fn record(record: PaintRecord) -> Self {
        Self(Rc::from([Command::Record(record)]))
    }

    /// Concatenate plans without reordering or deduplicating their records.
    pub fn sequence(plans: impl IntoIterator<Item = Self>) -> Self {
        Self(plans.into_iter().flat_map(|plan| plan.0.to_vec()).collect())
    }

    /// Wrap a whole plan in one ancestor scope, shared by all nested records.
    pub fn scope(descriptor: PaintScopeDescriptor, children: Self) -> Self {
        let mut commands = Vec::with_capacity(children.0.len() + 2);
        commands.push(Command::Begin(Box::new(descriptor)));
        commands.extend_from_slice(&children.0);
        commands.push(Command::End);
        Self(commands.into())
    }

    pub(super) fn validate(&self, window: &Window) -> Result<Bounds<Pixels>, PaintRecordError> {
        if !window.scale_factor().is_finite() || window.scale_factor() <= 0.0 {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let mut extent: Option<Bounds<Pixels>> = None;
        for command in self.0.iter() {
            let bounds = match command {
                Command::Record(record) => {
                    record.check(window)?;
                    record.0.clip
                }
                Command::Begin(descriptor) => descriptor.validate(window.scale_factor())?,
                Command::End => continue,
            };
            extent = Some(extent.map_or(bounds, |extent| extent.union(&bounds)));
        }
        let extent = extent.unwrap_or_default();
        if !validation::scaled_bounds_valid(extent, window.scale_factor()) {
            return Err(PaintRecordError::InvalidGeometry);
        }
        Ok(extent)
    }

    pub(super) fn paint(&self, window: &mut Window) {
        enum Frame<'a> {
            Clip,
            Group(Box<Scene>, Vec<ContentMask<Pixels>>, &'a PaintGroupDescriptor),
        }
        let input_mask = window.content_mask();
        let mut frames = Vec::new();
        for command in self.0.iter() {
            match command {
                Command::Record(record) => {
                    let mask = window.content_mask().scale(window.scale_factor());
                    let scene = record.0.scene.copy_record(Point::default(), Some(mask.bounds));
                    let scene = window.adopt_scene(scene);
                    window.next_frame.scene.replay(0..scene.paint_operations.len(), &scene);
                }
                Command::Begin(descriptor) => match descriptor.as_ref() {
                    PaintScopeDescriptor::Clip(bounds) => {
                        let mask = ContentMask { bounds: window.content_mask().bounds.intersect(bounds) };
                        window.content_mask_stack.push(mask);
                        frames.push(Frame::Clip);
                    }
                    PaintScopeDescriptor::Group(group) => {
                        let mut inner = Scene::default();
                        inner.in_group = true;
                        // Ancestor clips apply to the group output, not its filter input.
                        let masks = mem::replace(&mut window.content_mask_stack, vec![input_mask]);
                        frames.push(Frame::Group(Box::new(mem::replace(&mut window.next_frame.scene, inner)), masks, group));
                    }
                },
                Command::End => match frames.pop() {
                    Some(Frame::Clip) => { window.content_mask_stack.pop(); }
                    Some(Frame::Group(mut outer, masks, group)) => {
                        mem::swap(&mut window.next_frame.scene, &mut outer);
                        window.content_mask_stack = masks;
                        group.composite(window, *outer);
                    }
                    None => unreachable!("balanced private paint plan"),
                },
            }
        }
    }
}

impl PaintGroupDescriptor {
    fn composite(&self, window: &mut Window, scene: Scene) {
        window.paint_group(
            self.bounds, self.corner_radii, self.blur_radius, self.opacity,
            self.blend, &self.polygon, self.mask.clone(), self.mask_clip,
            |window| window.next_frame.scene = scene,
        );
        let scale = window.scale_factor();
        if let Some(last) = window.next_frame.scene.groups.last_mut() {
            last.polygon_inverse = self.polygon_inverse;
            last.projection = self.projection;
            last.fragments = self.fragments.iter().map(|fragment| {
                fragment.iter().map(|point| point.scale(scale)).collect()
            }).collect();
        }
    }
}
