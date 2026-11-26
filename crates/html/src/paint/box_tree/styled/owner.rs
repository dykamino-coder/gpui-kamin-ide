//! The painter a styled element produces. Diagnostic labels are written only at the counter.
use crate::dom::Element;
use crate::style::computed::BoxStyle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Own {
    Plain,
    Band,
    Not(Refusal),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    RootCanvas,
    Hidden,
    Clone,
    Filter,
    Decoration,
    BandEffect,
    BandPaint,
    BandNested,
    BandGeometry,
    Kind,
}

impl Refusal {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::RootCanvas => "root_canvas",
            Self::Hidden => "hidden",
            Self::Clone => "clone",
            Self::Filter => "filter",
            Self::Decoration => "decoration",
            Self::BandEffect => "band_effect",
            Self::BandPaint => "band_paint",
            Self::BandNested => "band_nested",
            Self::BandGeometry => "band_geometry",
            Self::Kind => "kind",
        }
    }
}

/// The box builder supplies the decorations it actually emitted. When planning an
/// edit, the same description checks whether the new output still fits the record.
/// `boxed`: the element is built as a box of its own (a `Div`), not as runs of a
/// paragraph: an inline element made a flex or grid item (blockified,
/// css-display-3 §2.7), an atomic inline. Its paint is then a box's, whatever its
/// `display` says, and a transform moves the box, not the pieces of a band.
pub(super) fn painter(
    e: &Element,
    c: &BoxStyle,
    boxed: bool,
    decorated: impl FnOnce() -> bool,
) -> Own {
    let band = !boxed && e.inline && c.reset_box_layout.display.is_none();
    if matches!(e.tag.as_str(), "html" | "body") {
        return Own::Not(Refusal::RootCanvas);
    }
    let effect = if c.inh.ui.hidden == Some(true) {
        Some(Refusal::Hidden)
    } else if c.reset_fragmentation.bdb_clone {
        Some(Refusal::Clone)
    } else if c.reset_effects.filter_ref.is_some() || c.reset_effects.backdrop_color.is_some() {
        Some(Refusal::Filter)
    } else if band
        && (c.reset_transforms.transform.is_some() || c.reset_transforms.transform_ancestor)
    {
        // Band pieces on a transformed line keep refusing.
        Some(Refusal::BandEffect)
    } else {
        // A transformed box's own run is painted again under the transform
        // it was recorded with (`gpui::Window::rerecord_layer_box`); a box in
        // a 3D context keeps no run, and the layer refuses it.
        None
    };
    if band {
        return if effect.is_some() {
            Own::Not(Refusal::BandEffect)
        } else {
            Own::Band
        };
    }
    match effect {
        Some(why) => Own::Not(why),
        None if decorated() => Own::Not(Refusal::Decoration),
        None => Own::Plain,
    }
}
