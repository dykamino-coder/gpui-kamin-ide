//! The size of the viewport as the screen medium of the document
//! (`Document::set_viewport`).
//!
//! `width`, `height`, `aspect-ratio` and `orientation` of a screen medium are
//! the viewport's (mediaqueries-4 §4.1, §4.2, §4.3, §4.4). A new size first
//! re-evaluates the media feature expressions of the build that read it
//! (`MediaReads`): if none changes its result, the rules of the document are
//! the same, and only the medium takes the new size. Otherwise the document
//! is restyled from its source DOM under the new medium, as a rebuild after a
//! mutation is. Blink does the same on a resize
//! (`LocalFrameView::ViewportSizeChanged` →
//! `StyleEngine::MediaQueryAffectingValueChanged`).

use super::mutation::Batch;
use crate::document::{Document, DocumentFailure};

impl Document {
    /// Change the host preference (Media Queries 5 section 12.5). Even without
    /// a media query, used color-scheme and system colors may change.
    pub fn set_preferred_color_scheme(&mut self, dark: bool) -> Result<bool, DocumentFailure> {
        let media = self.state.media();
        if media.dark == dark {
            return Ok(false);
        }
        self.state
            .values
            .media
            .set(crate::style::css::Media { dark, ..media });
        let mut batch = Batch::new(&[]).expect("an empty batch addresses no node");
        let theme_css = self.theme_css.clone();
        let revision = self.budget().revision();
        if let Err(error) = self.rebuild(&mut batch, None, &theme_css, self.key, revision) {
            self.state.values.media.set(media);
            return Err(error);
        }
        Ok(true)
    }

    /// Take `viewport` (CSS px) as the size of the screen medium. `true` —
    /// a media query changed its result and the document was restyled. A
    /// printed document keeps its page size.
    pub fn set_viewport(&mut self, viewport: (f32, f32)) -> Result<bool, DocumentFailure> {
        let media = self.state.media();
        if media.print || (media.width, media.height) == viewport {
            return Ok(false);
        }
        let next = crate::style::css::Media {
            width: viewport.0,
            height: viewport.1,
            ..media
        };
        let flips = self.media_reads.flips(&next, self.state.values());
        self.state.values.media.set(next);
        if !flips {
            return Ok(false);
        }
        let mut batch = Batch::new(&[]).expect("an empty batch addresses no node");
        let theme_css = self.theme_css.clone();
        let revision = self.budget().revision();
        // A flipped query restyles everything it may reach: no known change set.
        self.rebuild(&mut batch, None, &theme_css, self.key, revision)?;
        Ok(true)
    }
}
