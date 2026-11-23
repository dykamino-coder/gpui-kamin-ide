//! Detached value inputs shared by pure cascade and retained subtree reuse.
use crate::style::values::value::{RootMetrics, Scheme};
use crate::values::ValueCx;

pub(crate) struct Environment {
    root: RootMetrics,
    font_generation: u64,
    viewport: [u32; 2],
    media: (u32, u32, bool, bool),
    scheme: Scheme,
    quirks: bool,
    clearance: bool,
    frame: bool,
}

impl Environment {
    pub(crate) fn new(vcx: ValueCx<'_>) -> Self {
        let media = vcx.doc.media.get();
        Self {
            root: vcx.doc.root.borrow().clone(),
            font_generation: vcx.doc.font_generation(),
            viewport: [vcx.viewport.0.to_bits(), vcx.viewport.1.to_bits()],
            media: (
                media.width.to_bits(),
                media.height.to_bits(),
                media.dark,
                media.print,
            ),
            scheme: vcx.scheme,
            quirks: vcx.doc.quirks(),
            clearance: vcx.clearance_possible,
            frame: vcx.frame,
        }
    }

    pub(crate) fn matches(&self, vcx: ValueCx<'_>) -> bool {
        let media = vcx.doc.media.get();
        self.font_generation == vcx.doc.font_generation()
            && self.viewport == [vcx.viewport.0.to_bits(), vcx.viewport.1.to_bits()]
            && self.media
                == (
                    media.width.to_bits(),
                    media.height.to_bits(),
                    media.dark,
                    media.print,
                )
            && self.scheme == vcx.scheme
            && self.quirks == vcx.doc.quirks()
            && self.clearance == vcx.clearance_possible
            && self.frame == vcx.frame
            && self.root.same_inputs(&vcx.doc.root.borrow())
    }
}

impl PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        self.font_generation == other.font_generation
            && self.viewport == other.viewport
            && self.media == other.media
            && self.scheme == other.scheme
            && self.quirks == other.quirks
            && self.clearance == other.clearance
            && self.frame == other.frame
            && self.root.same_inputs(&other.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::css::Media;
    use crate::values::DocumentValues;

    #[track_caller]
    fn rejects(snapshot: &Environment, vcx: ValueCx<'_>) {
        assert!(!snapshot.matches(vcx));
        assert!(*snapshot != Environment::new(vcx));
    }

    #[test]
    fn document_and_value_inputs_invalidate_both_cache_consumers() {
        let values = DocumentValues::detached(Media::default());
        let original = Environment::new(values.values());
        assert!(original.matches(values.values()));
        assert!(original == Environment::new(values.values()));
        let mut vcx = values.values();
        vcx.viewport.0 = 800.0;
        rejects(&original, vcx);
        rejects(&original, values.frame_values());
        vcx = values.values();
        vcx.clearance_possible = false;
        rejects(&original, vcx);
        vcx = values.values();
        vcx.scheme = Scheme::new(!values.media.get().dark);
        rejects(&original, vcx);
        values.quirks.set(true);
        rejects(&original, values.values());
        values.quirks.set(false);
        values.root.borrow_mut().font_px = 24.0;
        rejects(&original, values.values());
        values.root.borrow_mut().font_px = 16.0;
        assert!(original.matches(values.values()));
        let mut media = values.media.get();
        media.print = !media.print;
        values.media.set(media);
        rejects(&original, values.values());
        media.print = !media.print;
        values.media.set(media);
        assert!(original.matches(values.values()));
        values.font_inputs_changed();
        rejects(&original, values.values());
    }
}
