//! Keep native styled elements on the caller's layout shaping policy.

use super::StyledText;

impl StyledText {
    /// Choose whether differently styled runs break ligatures.
    ///
    /// Enabled by default for IDE text. Browser callers should disable it for
    /// CSS boundary shaping, including when selection splits decoration runs.
    pub fn with_ligature_breaking(mut self, enabled: bool) -> Self {
        self.break_ligatures = enabled;
        self
    }
}
