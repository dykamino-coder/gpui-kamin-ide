//! Containment excludes descendant intrinsic widths, including HTML block boxes routed through flex.

use crate::style::{AvailableSpace, Contain};
use crate::tree::{Baselines, LayoutOutput};
use crate::util::MaybeMath;

pub(super) fn intrinsic_width(
    known: Option<f32>,
    empty: Option<f32>,
    available: AvailableSpace,
    edges: f32,
    min: Option<f32>,
    max: Option<f32>,
) -> Option<f32> {
    // CSS Containment 2 §3.1: treat content as empty, while retaining the
    // box's authored size, constraints and decorations. Definite available
    // space still stretches an auto block width normally (CSS2 §10.3.3).
    known.or_else(|| match (empty, available) {
        (Some(size), AvailableSpace::MinContent | AvailableSpace::MaxContent) => {
            Some((size + edges).maybe_clamp(min, max).max(edges))
        }
        _ => None,
    })
}

pub(super) fn suppress_baselines(output: &mut LayoutOutput, contain: Contain) {
    if contain.suppresses_baseline() {
        output.baselines = Baselines::NONE;
        output.baselines_x = Baselines::NONE;
    }
}
