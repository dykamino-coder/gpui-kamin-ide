//! KaminIDE patch: the redraw rectangles of a frame drawn in one pass from
//! the instances that reach them (`cull`).
//!
//! The frame's batches are walked in paint order, retained layers expanded
//! at their place, exactly as a whole frame draws them. Instance batches
//! leave only their gathered copies; a batch whose copies follow a run of the
//! same kind (and atlas texture) without anything drawn between joins that
//! run's draw. Paths and surfaces (group composites, backdrop passes) read
//! or write beyond their instances, so they stay rectangle by rectangle under
//! the scissor, as before, without the group composites whose region misses
//! the rectangle (`composite_region`) and the path batches that miss it
//! (`path_pass`). A layer drawn this way keeps its cached buffers
//! for the next whole frame (`LayerCache::keep`).

use super::super::{
    DirectXRenderer, batch_area, group_batch::LayerInstances, group_region::Region,
};
use super::{bounds, disjoint, extent, gather, region};
use anyhow::Result;
use gpui::{AtlasTextureId, ContentMask, GradientData, PrimitiveBatch, Quad, ScaledPixels, Scene};
use std::ops::Range;

/// What one draw of the pass draws: an instance kind and its atlas texture.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Shadows,
    Quads,
    Underlines,
    Mono(AtlasTextureId),
    Subpixel(AtlasTextureId),
    Poly(AtlasTextureId),
}

impl Kind {
    /// The batch kind of the frame instrumentation (`batch_area`).
    fn index(self) -> usize {
        match self {
            Kind::Shadows => 0,
            Kind::Quads => 1,
            Kind::Underlines => 3,
            Kind::Mono(_) => 4,
            Kind::Subpixel(_) => 5,
            Kind::Poly(_) => 6,
        }
    }
}

enum Op<'s> {
    /// Gathered instances `range` of one kind; `area` pixels they cover.
    Draw {
        kind: Kind,
        range: Range<usize>,
        area: f64,
    },
    /// A batch drawn within each rectangle in turn.
    Each(&'s Scene, PrimitiveBatch),
    /// Gathered instances `range` of one kind, uncut, drawn within each
    /// rectangle in turn under the scissor (`gather_runs`).
    Scissored { kind: Kind, range: Range<usize> },
}

/// The pass being gathered.
struct Gather<'s> {
    ops: Vec<Op<'s>>,
    /// No quad batch was met yet: the first one's quad 0 may be the clear.
    first_quads: bool,
    backdrop: bool,
    /// Where each scene's gradients start in the pass's gradient buffer.
    bases: Vec<(*const Scene, u32)>,
}

impl Gather<'_> {
    fn push(&mut self, kind: Kind, (n, area): (usize, f64), end: usize) {
        if n == 0 {
            return;
        }
        if let Some(Op::Draw {
            kind: k,
            range,
            area: a,
        }) = self.ops.last_mut()
            && *k == kind
        {
            range.end = end;
            *a += area;
            return;
        }
        self.ops.push(Op::Draw {
            kind,
            range: end - n..end,
            area,
        });
    }

    /// Gather `items` in paint order: a run whose extent is known as cut
    /// copies (`gather`), a run that a transformation moves (no extent) once
    /// each with its own mask, drawn within each rectangle under the scissor.
    /// A mask cut through a transformed instance clips its triangles anew
    /// (`SV_ClipDistance`, clipped as geometry on WARP), moving the edge
    /// pixels the whole frame draws; the scissor tests pixels only.
    fn gather_runs<T: Copy>(
        &mut self,
        kind: Kind,
        out: &mut Vec<T>,
        items: &[T],
        rects: &[[f32; 4]],
        extent: impl Fn(&T) -> Option<[f32; 4]>,
        mask: impl Fn(&mut T) -> &mut ContentMask<ScaledPixels>,
    ) {
        let mut at = 0;
        while at < items.len() {
            let moved = extent(&items[at]).is_none();
            let run = items[at..].iter().take_while(|item| extent(item).is_none() == moved).count();
            let part = &items[at..at + run];
            if moved {
                let start = out.len();
                out.extend_from_slice(part);
                match self.ops.last_mut() {
                    Some(Op::Scissored { kind: k, range }) if *k == kind && range.end == start => range.end = out.len(),
                    _ => self.ops.push(Op::Scissored { kind, range: start..out.len() }),
                }
            } else {
                let got = gather(out, part, rects, &extent, &mask);
                self.push(kind, got, out.len());
            }
            at += run;
        }
    }

    fn gradient_base(&mut self, scene: &Scene, gradients: &mut GradientData) -> u32 {
        let key = scene as *const Scene;
        if let Some(&(_, base)) = self.bases.iter().find(|(s, _)| *s == key) {
            return base;
        }
        let base = gradients.descriptors.len() as u32;
        gradients.append(scene, &mut []);
        self.bases.push((key, base));
        base
    }
}

impl DirectXRenderer {
    /// Draw `scene` within `rects` (cleared first) in one pass. The scissor
    /// goes on only now: the group passes and their blurs write other
    /// targets in their own coordinates.
    pub(in super::super) fn draw_scene_within(
        &mut self,
        scene: &Scene,
        rects: &[Region],
        clear_color: [f32; 4],
    ) -> Result<()> {
        let rects = disjoint(rects);
        let Some(all) = bounds(&rects) else {
            return Ok(());
        };
        let mut instances = std::mem::take(&mut self.blur.layer_instances);
        instances.clear();
        let mut g = Gather {
            ops: Vec::new(),
            first_quads: true,
            backdrop: self.backdrop.is_some(),
            bases: Vec::new(),
        };
        self.gather_scene(scene, &rects, &mut instances, &mut g);
        let state = self.begin_scissor(all)?;
        let clear = *self.backdrop.as_ref().unwrap_or(&clear_color);
        let drawn = self.draw_gathered(&rects, all, clear, &instances, g.ops);
        self.blur.frame_within = None;
        self.backdrop_pending = false;
        unsafe { self.dev().device_context.RSSetState(&state) };
        instances.clear();
        self.blur.layer_instances = instances;
        drawn
    }

    fn gather_scene<'s>(
        &mut self,
        scene: &'s Scene,
        rects: &[[f32; 4]],
        all: &mut LayerInstances,
        g: &mut Gather<'s>,
    ) {
        for batch in scene.batches() {
            match batch {
                PrimitiveBatch::Layer(index) => {
                    if let Some(layer) = scene.layer(index) {
                        self.layer_cache.keep(layer.id());
                        self.gather_scene(layer.scene(), rects, all, g);
                    }
                }
                PrimitiveBatch::Paths(_) | PrimitiveBatch::Surfaces(_) => {
                    g.ops.push(Op::Each(scene, batch))
                }
                PrimitiveBatch::Shadows(r) => {
                    let got = gather(
                        &mut all.shadows,
                        &scene.shadows[r],
                        rects,
                        shadow_extent,
                        |s| &mut s.content_mask,
                    );
                    g.push(Kind::Shadows, got, all.shadows.len());
                }
                PrimitiveBatch::Quads(r) => {
                    // The frame clear drew the first quad of a frame with a backdrop.
                    let skip = std::mem::take(&mut g.first_quads) && g.backdrop && r.start == 0;
                    let start = all.quads.len();
                    let items = &scene.quads[r.start + usize::from(skip)..r.end];
                    g.gather_runs(
                        Kind::Quads,
                        &mut all.quads,
                        items,
                        rects,
                        |q| extent(q.bounds, &q.content_mask, Some(&q.transformation), 1.0),
                        |q| &mut q.content_mask,
                    );
                    if all.quads[start..].iter().any(Quad::is_analytic_gradient) {
                        let base = g.gradient_base(scene, &mut all.gradients);
                        all.quads[start..]
                            .iter_mut()
                            .for_each(|q| q.offset_gradient(base));
                    }
                }
                PrimitiveBatch::Underlines(r) => {
                    let got = gather(
                        &mut all.underlines,
                        &scene.underlines[r],
                        rects,
                        |u| extent(u.bounds, &u.content_mask, None, 1.0),
                        |u| &mut u.content_mask,
                    );
                    g.push(Kind::Underlines, got, all.underlines.len());
                }
                PrimitiveBatch::MonochromeSprites { texture_id, range } => {
                    let items = &scene.monochrome_sprites[range];
                    g.gather_runs(
                        Kind::Mono(texture_id),
                        &mut all.mono,
                        items,
                        rects,
                        |s| extent(s.bounds, &s.content_mask, Some(&s.transformation), 1.0),
                        |s| &mut s.content_mask,
                    );
                }
                PrimitiveBatch::SubpixelSprites { texture_id, range } => {
                    let items = &scene.subpixel_sprites[range];
                    g.gather_runs(
                        Kind::Subpixel(texture_id),
                        &mut all.subpixel,
                        items,
                        rects,
                        |s| extent(s.bounds, &s.content_mask, Some(&s.transformation), 1.0),
                        |s| &mut s.content_mask,
                    );
                }
                PrimitiveBatch::PolychromeSprites { texture_id, range } => {
                    let items = &scene.polychrome_sprites[range];
                    g.gather_runs(
                        Kind::Poly(texture_id),
                        &mut all.poly,
                        items,
                        rects,
                        |s| extent(s.bounds, &s.content_mask, Some(&s.transformation), 1.0),
                        |s| &mut s.content_mask,
                    );
                }
            }
        }
    }

    fn draw_gathered(
        &mut self,
        rects: &[[f32; 4]],
        all: Region,
        clear: [f32; 4],
        instances: &LayerInstances,
        ops: Vec<Op>,
    ) -> Result<()> {
        self.restore_frame_globals()?;
        for r in rects {
            self.set_scissor(region(r));
            self.clear_frame(clear)?;
        }
        self.set_scissor(all);
        self.upload_instances(instances)?;
        // The clear drew the backdrop quad: it was left out of the gathered quads.
        self.backdrop_pending = false;
        for op in ops {
            let cost = match &op {
                Op::Draw { kind, range, area } => (kind.index(), range.len(), *area),
                Op::Scissored { kind, range } => (kind.index(), range.len() * rects.len(), 0.0),
                Op::Each(scene, batch) => batch_area(scene, batch),
            };
            match op {
                Op::Draw { kind, range, .. } => self.draw_kind(kind, range, instances)?,
                Op::Scissored { kind, range } => {
                    for r in rects {
                        self.set_scissor(region(r));
                        self.draw_kind(kind, range.clone(), instances)?;
                    }
                    self.set_scissor(all);
                }
                Op::Each(scene, batch) => {
                    for r in rects {
                        self.set_scissor(region(r));
                        self.blur.frame_within = Some(region(r));
                        match &batch {
                            // Works within the rectangle only (`path_pass`).
                            PrimitiveBatch::Paths(range) => {
                                self.draw_paths(&scene.paths[range.clone()])?
                            }
                            PrimitiveBatch::Surfaces(range) => {
                                // A group composite writes only within its
                                // region: one that misses the rectangle is left out.
                                let within: Vec<_> = scene.surfaces[range.clone()]
                                    .iter()
                                    .filter(|s| {
                                        s.group == 0 || self.composite_region(s).meets(region(r))
                                    })
                                    .cloned()
                                    .collect();
                                if !within.is_empty() {
                                    self.draw_surfaces(&within)?;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.blur.frame_within = None;
                    self.set_scissor(all);
                }
            }
            if self.timing_main {
                let (kind, prims, area) = cost;
                self.gpu_timer
                    .instrumentation
                    .batches
                    .count(kind, prims, area);
                let (device, ctx) = (self.dev().device.clone(), self.dev().device_context.clone());
                self.gpu_timer.mark(&device, &ctx, kind);
            }
        }
        Ok(())
    }
}

impl DirectXRenderer {
    /// Draw the gathered instances `range` of `kind`.
    fn draw_kind(&mut self, kind: Kind, range: Range<usize>, instances: &LayerInstances) -> Result<()> {
        let (start, len) = (range.start, range.len());
        match kind {
            Kind::Shadows => self.draw_shadows(start, len),
            Kind::Quads => self.draw_quads(&instances.quads[range], start),
            Kind::Underlines => self.draw_underlines(start, len),
            Kind::Mono(t) => self.draw_monochrome_sprites(t, start, len),
            Kind::Subpixel(t) => self.draw_subpixel_sprites(t, start, len),
            Kind::Poly(t) => self.draw_polychrome_sprites(t, start, len),
        }
    }
}

/// The pixels a shadow can write (`shadow_vertex`): an inset one its
/// element's box, a drop shadow its box with the gaussian tail.
fn shadow_extent(s: &gpui::Shadow) -> Option<[f32; 4]> {
    if s.inset != 0 {
        return extent(s.element_bounds, &s.content_mask, None, 1.0);
    }
    extent(
        s.bounds,
        &s.content_mask,
        None,
        3.0 * s.blur_radius.0.max(0.0) + 1.0,
    )
}
