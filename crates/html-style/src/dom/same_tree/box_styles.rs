//! The style stages of one box, kept for comparison with a later build.

use super::{Element, SharedStyle, layers};

/// The style stages of one box: its style and the pseudo-element layers,
/// which most boxes do not have. Shared stages are pointers into the
/// styles of the build they were taken from.
#[derive(Clone)]
pub struct BoxStyles {
    pub(super) style: SharedStyle,
    pub(super) layers: Option<Box<[Option<SharedStyle>; 3]>>,
}

impl BoxStyles {
    pub fn of(e: &Element) -> Self {
        let layers = layers(e).map(Option::<&SharedStyle>::cloned);
        let layers = layers.iter().any(Option::is_some).then(|| Box::new(layers));
        let style = e.style.clone();
        Self { style, layers }
    }

    /// The element `shell` with these style stages.
    pub fn apply(&self, shell: &Element) -> Element {
        let [hover, first_letter, first_line] = self.layers.as_deref().cloned().unwrap_or_default();
        Element {
            style: self.style.clone(),
            hover,
            first_letter,
            first_line,
            ..shell.clone()
        }
    }

    pub(super) fn same(&self, b: &Element) -> bool {
        let layers = self
            .layers
            .as_deref()
            .map(|l| l.each_ref().map(Option::as_ref));
        Self::same_as(&self.style, layers.unwrap_or_default(), b)
    }

    pub(super) fn same_as(
        style: &SharedStyle,
        own: [Option<&SharedStyle>; 3],
        b: &Element,
    ) -> bool {
        let layer = |x: Option<&SharedStyle>, y: Option<&SharedStyle>| match (x, y) {
            (None, None) => true,
            (Some(x), Some(y)) => x.same(y),
            _ => false,
        };
        style.same(&b.style) && own.into_iter().zip(layers(b)).all(|(x, y)| layer(x, y))
    }

    pub(super) fn same_layout(&self, b: &Element) -> bool {
        let layers = self
            .layers
            .as_deref()
            .map(|l| l.each_ref().map(Option::as_ref));
        Self::same_stages_as(
            &self.style,
            layers.unwrap_or_default(),
            b,
            SharedStyle::same_layout,
        )
    }

    pub(super) fn same_stages_as(
        style: &SharedStyle,
        own: [Option<&SharedStyle>; 3],
        b: &Element,
        same: fn(&SharedStyle, &SharedStyle) -> bool,
    ) -> bool {
        let layer = |x: Option<&SharedStyle>, y: Option<&SharedStyle>| match (x, y) {
            (None, None) => true,
            (Some(x), Some(y)) => same(x, y),
            _ => false,
        };
        same(style, &b.style) && own.into_iter().zip(layers(b)).all(|(x, y)| layer(x, y))
    }
}
