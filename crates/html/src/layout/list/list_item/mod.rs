//! List item boxes paint their markers independently of the parent element tag.

use crate::dom::Element;
use crate::render::RenderOpts;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use gpui::{AnyElement, Styled};
mod styled;
pub(crate) use styled::inherited_style;
pub(crate) use styled::render_with_style;

fn shrink0(d: gpui::Div, li: &Element, parent: &Computed) -> gpui::Div {
    let flex_parent = matches!(
        parent.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
    );
    if li.style.flex_shrink.is_none() && !flex_parent {
        d.flex_shrink_0()
    } else {
        d
    }
}

/// CSS Lists 3 §3.1.1 UA sheet: `::marker { font-variant-numeric: tabular-nums }`.
/// It overrides the item's inherited numeric variant; only an author
/// `::marker` rule naming a numeric variant replaces it.
fn tabular_marker(style: &mut Computed, item: &Computed, layer: Option<&Computed>) {
    const NUMERIC: [&str; 8] = [
        "lnum", "onum", "pnum", "tnum", "frac", "afrc", "ordn", "zero",
    ];
    let numeric = |t: &str| NUMERIC.contains(&t);
    if layer.is_some_and(|m| m.font_features.iter().any(|(t, _)| numeric(t))) {
        return;
    }
    let mut features = if style.font_features.is_empty() {
        item.font_features.clone()
    } else {
        std::mem::take(&mut style.font_features)
    };
    features.retain(|(t, _)| !numeric(t));
    features.push(("tnum".into(), 1));
    style.font_features = features;
}

pub(super) fn render(li: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let mut merged = inherit(inherited, &li.style);
    if matches!(
        li.style.display,
        None | Some(Display::Block | Display::ListItem | Display::TableCell)
    ) || (li.style.display == Some(Display::InlineBlock)
        && li.style.inline_display != Some(true))
    {
        crate::render::pseudo_line_layers::install_first_letter(li, &mut merged);
    }
    render_with_style(li, inherited, &merged, opts)
}
