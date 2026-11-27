//! Root paint ownership is a raster input, independent of SVG source attributes.

use crate::cache::FloatKey;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RootPaint {
    #[default]
    Svg,
    /// The CSS replaced box paints the background and its computed opacity.
    /// The opacity remains the root's value for explicit SVG inheritance.
    CssBox { opacity: Option<FloatKey> },
}

impl RootPaint {
    pub fn opacity(self) -> Option<resvg::usvg::Opacity> {
        match self {
            Self::CssBox {
                opacity: Some(value),
            } => Some(resvg::usvg::Opacity::new_clamped(value.get())),
            _ => None,
        }
    }
}
