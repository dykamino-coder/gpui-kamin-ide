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

impl TextFonts {
    /// Documents use scale 1 until bound to a window.
    pub(crate) fn device_scale(&self) -> f32 {
        match self.metrics.scale.get() {
            0.0 => 1.0,
            scale => scale,
        }
    }

    /// A different metric grid invalidates layout records and font-unit facts.
    pub(crate) fn set_device_scale(&self, scale: f32, vcx: crate::values::ValueCx<'_>) {
        debug_assert!(scale.is_finite() && scale > 0.0);
        if self.device_scale() != scale {
            self.metrics.scale.set(scale);
            vcx.doc.font_inputs_changed();
        }
    }
}
