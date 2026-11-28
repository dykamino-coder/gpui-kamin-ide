//! Host control and observation of the document's animation sample.

use super::Document;
use crate::animation::clock::AnimationFrame;
use std::time::Duration;

impl Document {
    /// Override effect elapsed for deterministic snapshots; `None` restores live time.
    ///
    /// Snapshot keyframes use this absolute elapsed, independent of when their
    /// element first mounted. Transitions retain their input-event origin.
    /// Changing the input invalidates retained output. The host requests a draw;
    /// a stopped snapshot does not request an animation loop itself.
    pub fn set_animation_elapsed(&self, elapsed: Option<Duration>) {
        self.state.animation.set_elapsed(elapsed);
        self.state.scrolling.set_elapsed(elapsed);
    }

    /// Read animation work in the last prepared window frame without advancing
    /// time, clearing counters or scheduling another frame. `None` means that no
    /// frame has been prepared since the last elapsed-input change.
    pub fn animation_frame(&self) -> Option<AnimationFrame> {
        self.state.animation.frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn animation_input_survives_text_adoption_and_rebuilt_revisions() {
        let mut doc = Document::builder("<p>one</p>", "").build().unwrap();
        let clock = doc.state.animation.clone();
        doc.set_animation_elapsed(Some(Duration::from_millis(375)));
        let key = clock.retained_key();
        assert!(doc.update("<p>two</p>", "").unwrap().changed());
        assert!(Rc::ptr_eq(&clock, &doc.state.animation));
        assert_eq!(doc.state.animation.retained_key(), key);
        assert!(
            doc.update("<section><p>three</p></section>", "p {color:red}")
                .unwrap()
                .changed()
        );
        assert!(Rc::ptr_eq(&clock, &doc.state.animation));
        assert_eq!(doc.state.animation.retained_key(), key);
        let independent = Document::builder("<p>two</p>", "").build().unwrap();
        assert!(!Rc::ptr_eq(&clock, &independent.state.animation));
        assert_eq!(independent.state.animation.retained_key().0, None);
    }

    #[test]
    fn equal_inputs_are_idempotent_and_live_restoration_changes_generation() {
        let doc = Document::builder("<p>one</p>", "").build().unwrap();
        doc.set_animation_elapsed(Some(Duration::ZERO));
        let key = doc.state.animation.retained_key();
        doc.set_animation_elapsed(Some(Duration::ZERO));
        assert_eq!(doc.state.animation.retained_key(), key);
        assert_eq!(doc.animation_frame(), None);
        doc.set_animation_elapsed(None);
        assert_eq!(doc.state.animation.retained_key().0, None);
        assert_ne!(doc.state.animation.retained_key().1, key.1);
    }
}
