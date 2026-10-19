//! Stable clamp identity for children rebuilt during band measurement.

use crate::layout::float::band_flow::Build;
use crate::text::clamp::DeferredClamp;
use std::rc::Rc;

pub(super) fn deferred(build: Build, reserve: bool) -> Build {
    let Some(scope) = DeferredClamp::capture() else {
        return build;
    };
    // Assign paragraph numbers in tree order, before later siblings are built.
    // Probe copies reuse this scope; their sequence never escapes the build.
    if reserve {
        drop(build(0.0, 0.0, None, None));
    }
    Rc::new(move |cb, avail, shapes, height| {
        let _scope = scope.enter();
        build(cb, avail, shapes, height)
    })
}
