//! The DevTools `why` record of a paint-only write that found no scene
//! layer to write into: the layer's own refusal, or the record missing.

use crate::document::Document;

impl Document {
    /// Why `write_paint_values` had no layer (DevTools `why`): the layer's own
    /// refusal at the line that made it, or the record this revision lacks.
    pub(super) fn note_no_layer(&self) {
        if !crate::why::enabled() {
            return;
        }
        match (self.reuse.scene_layer().is_some(), self.reuse.no_layer()) {
            (false, Some(why)) => {
                let code = match why.reason {
                    "environment" => "no_layer.painted_environment",
                    "ink" => "no_layer.painted_ink",
                    "nested" => "no_layer.painted_nested",
                    "dropped" => "no_layer.dropped",
                    _ => "no_layer.painted_other",
                };
                crate::why::note_at(code, why.site, || {
                    format!("no scene layer: {}", why.reason)
                });
            }
            (false, None) => crate::why::note("no_layer.unrecorded", || {
                "no frame painted into a scene layer yet".into()
            }),
            (true, _) => crate::why::note("no_layer.frame_unrecorded", || {
                match self.unpublished_reason() {
                    Some(code) => format!("this revision's frame was not published: {code}"),
                    None => "no frame was recorded for this revision yet".into(),
                }
            }),
        }
    }
}
