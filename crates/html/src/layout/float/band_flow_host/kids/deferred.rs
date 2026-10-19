//! Stable explicit clamp identity for children rebuilt during band measurement.

use crate::layout::float::band_flow::{Build, Shapes};
use crate::render::RenderOpts;
use crate::text::clamp::DeferredClamp;
use gpui::AnyElement;
use std::rc::Rc;

pub(super) fn deferred(
    opts: &RenderOpts,
    reserve: bool,
    build: impl Fn(&RenderOpts, f32, f32, Option<Shapes>, Option<f32>) -> AnyElement + 'static,
) -> Build {
    let scope = DeferredClamp::capture(&opts.cx);
    // Reserve numbers before later siblings, then replay the captured sequence.
    if scope.is_some() && reserve {
        drop(build(opts, 0.0, 0.0, None, None));
    }
    let opts = opts.clone();
    Rc::new(move |cb, avail, shapes, height, inline_measure| {
        let probe = opts.inner(|cx| {
            if let Some(measure) = &inline_measure {
                cx.inline_measure = Some(measure.clone());
            }
            if let Some(scope) = &scope {
                cx.clamp_build = scope.build();
            }
        });
        build(&probe, cb, avail, shapes, height)
    })
}
