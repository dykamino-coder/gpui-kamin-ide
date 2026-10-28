//! Per-document font state of the text backend: the platform font context, measurement probes
//! and their caches (`Metrics`) and parsed MATH tables (`MathFonts`). The document owns one
//! shared instance; builds reach it through `LayoutCx::text_fonts`, the document's font-facts
//! backend retains the same `Rc`. The @font-face registry, the font input generation and the
//! document family belong to the document font state, not here.

use crate::text::fonts::math::MathFonts;
use crate::text::metrics::Metrics;

#[derive(Default)]
pub(crate) struct TextFonts {
    /// Platform font context, measurement probes and measured fractions.
    pub(crate) metrics: Metrics,
    /// Parsed MATH sources and registered native-family data.
    pub(crate) math: MathFonts,
}
