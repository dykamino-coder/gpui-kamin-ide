//! Immutable gradient stops live as long as their retained paint record.
//! Descriptor indices belong to a scene and are remapped when commands move.

use super::{PaintOperation, Primitive, Quad, Scene};
use std::sync::Arc;

/// Resolved sRGB gradient, shared by content rather than by its painted area.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AnalyticGradient {
    geometry: [u32; 4],
    css_size: [u32; 2],
    radial: bool,
    repeating: bool,
    stops: Box<[[u32; 5]]>,
}

impl AnalyticGradient {
    /// Geometry is [dx,dy,length,0] for linear, [cx,cy,rx,ry] for radial.
    /// Stops are straight sRGB [r,g,b,a,offset], in resolved CSS order.
    pub fn new(
        geometry: [f32; 4],
        css_size: [f32; 2],
        radial: bool,
        repeating: bool,
        stops: impl IntoIterator<Item = [f32; 5]>,
    ) -> Option<Arc<Self>> {
        let stops: Vec<_> = stops.into_iter().collect();
        if stops.is_empty()
            || !geometry.iter().chain(&css_size).all(|v| v.is_finite())
            || css_size.iter().any(|v| *v <= 0.0)
            || !stops.iter().flatten().all(|v| v.is_finite())
            || stops.windows(2).any(|s| s[0][4] > s[1][4])
        {
            return None;
        }
        Some(Arc::new(Self {
            geometry: geometry.map(f32::to_bits),
            css_size: css_size.map(f32::to_bits),
            radial,
            repeating,
            stops: stops.into_iter().map(|s| s.map(f32::to_bits)).collect(),
        }))
    }
}

/// A quad and the immutable resource required to replay it in another scene.
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub struct GradientQuad {
    pub quad: Quad,
    pub gradient: Arc<AnalyticGradient>,
}

/// Shader layout, independent of the number of color stops.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
#[allow(missing_docs)]
pub struct GradientDescriptor {
    pub geometry: [f32; 4],
    pub css_size: [f32; 2],
    pub radial: u32,
    pub repeating: u32,
    pub first: u32,
    pub count: u32,
    pub pad: [u32; 2],
}

/// One straight-alpha sRGB stop in the shader buffer.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
#[allow(missing_docs)]
pub struct GradientStop {
    pub color: [f32; 4],
    pub offset: f32,
    pub pad: [u32; 3],
}

/// CPU staging buffers for one scene or one batch of group scenes.
#[derive(Default)]
#[allow(missing_docs)]
pub struct GradientData {
    pub descriptors: Vec<GradientDescriptor>,
    pub stops: Vec<GradientStop>,
}

impl GradientData {
    /// Append scene resources and remap its copied quads to this batch.
    pub fn append(&mut self, scene: &Scene, quads: &mut [Quad]) {
        let base = self.descriptors.len() as u32;
        self.descriptors.resize(
            self.descriptors.len() + scene.book.gradients.len(),
            Default::default(),
        );
        for (gradient, &index) in &scene.book.gradients {
            self.descriptors[base as usize + index as usize] = GradientDescriptor {
                geometry: gradient.geometry.map(f32::from_bits),
                css_size: gradient.css_size.map(f32::from_bits),
                radial: u32::from(gradient.radial),
                repeating: u32::from(gradient.repeating),
                first: self.stops.len() as u32,
                count: gradient.stops.len() as u32,
                pad: [0; 2],
            };
            self.stops
                .extend(gradient.stops.iter().map(|s| GradientStop {
                    color: [s[0], s[1], s[2], s[3]].map(f32::from_bits),
                    offset: f32::from_bits(s[4]),
                    pad: [0; 3],
                }));
        }
        quads.iter_mut().for_each(|q| q.offset_gradient(base));
    }
}

impl Quad {
    /// A copy of this quad drawn with its scene's gradients appended at
    /// `base` of a larger buffer (`GradientData::append`).
    pub fn offset_gradient(&mut self, base: u32) {
        if self.is_analytic_gradient() {
            self.background.stop_count += base;
        }
    }

    /// Whether this quad addresses the scene's analytic gradient buffer.
    pub fn is_analytic_gradient(&self) -> bool {
        self.background.tag == crate::color::BackgroundTag::AnalyticGradient
    }
}

impl Scene {
    pub(crate) fn gradient(&self, id: u32) -> Option<&Arc<AnalyticGradient>> {
        self.book.gradient_values.get(id as usize)
    }

    pub(crate) fn same_gradient_quad(&self, a: &Quad, other: &Scene, b: &Quad) -> bool {
        if !a.is_analytic_gradient() || !b.is_analytic_gradient() {
            return false;
        }
        let mut x = *a;
        x.order = b.order;
        x.background.stop_count = b.background.stop_count;
        x == *b
            && self
                .gradient(a.background.stop_count)
                .zip(other.gradient(b.background.stop_count))
                .is_some_and(|(x, y)| x == y)
    }

    /// Upload data, preserving scene-local indices even after depth sorting.
    pub fn gradient_data(&self) -> GradientData {
        let mut data = GradientData::default();
        data.append(self, &mut []);
        data
    }

    pub(crate) fn register_gradient(&mut self, p: &mut GradientQuad) {
        p.quad.background.tag = crate::color::BackgroundTag::AnalyticGradient;
        let next = self.book.gradients.len() as u32;
        p.quad.background.stop_count = *self
            .book
            .gradients
            .entry(p.gradient.clone())
            .or_insert_with(|| {
                self.book.gradient_values.push(p.gradient.clone());
                next
            });
    }

    /// A replaced run releases its last payload; no dead resources accumulate in a layer.
    pub(crate) fn compact_gradients(&mut self) {
        let old = std::mem::take(&mut self.book.gradients);
        self.book.gradient_values.clear();
        let mut remap = vec![0; old.len()];
        for op in &mut self.paint_operations {
            if let PaintOperation::Primitive(Primitive::Gradient(p)) = op {
                let next = self.book.gradients.len() as u32;
                let new = *self
                    .book
                    .gradients
                    .entry(p.gradient.clone())
                    .or_insert_with(|| {
                        self.book.gradient_values.push(p.gradient.clone());
                        next
                    });
                remap[p.quad.background.stop_count as usize] = new;
                p.quad.background.stop_count = new;
            }
        }
        for q in self.quads.iter_mut().filter(|q| q.is_analytic_gradient()) {
            q.background.stop_count = remap[q.background.stop_count as usize];
        }
    }
}

const _: () = assert!(std::mem::size_of::<GradientDescriptor>() == 48);
const _: () = assert!(std::mem::size_of::<GradientStop>() == 32);

#[cfg(test)]
mod tests;
