//! Стабильная явная идентичность clamp для потомков, перестраиваемых при измерении band.

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
    let retained = opts.cx.positioning.retain();
    // Reserve numbers before later siblings, then replay the captured sequence.
    if scope.is_some() && reserve {
        let (positioning, _escaped) = retained.isolate();
        let probe = opts.inner(|cx| cx.positioning = positioning);
        drop(build(&probe, 0.0, 0.0, None, None));
    }
    let opts = opts.clone();
    Rc::new(move |cb, avail, shapes, height, inline_measure| {
        let (positioning, escaped) = retained.isolate();
        let probe = opts.inner(|cx| {
            cx.positioning = positioning;
            if let Some(measure) = &inline_measure {
                cx.inline_measure = Some(measure.clone());
            }
            if let Some(scope) = &scope {
                cx.clamp_build = scope.build();
            }
        });
        escaped.wrap(build(&probe, cb, avail, shapes, height))
    })
}
