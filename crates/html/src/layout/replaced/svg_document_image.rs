//! An SVG image whose content needs the HTML engine (`<foreignObject>`).
//!
//! An `<img>` pointing at an SVG file is an independent document in the
//! secure static mode (SVG 2 §conform-referencing-modes, HTML §4.8.4.3): no
//! script runs, but `<foreignObject>` still renders its HTML. resvg skips
//! `<foreignObject>` entirely, so such a file is laid out the way a
//! standalone `.svg` document is (an SVG root in an embedded document with
//! the image's natural size, like Blink's `SVGImage` page) instead of being
//! rasterized. Images without `<foreignObject>` keep the raster path.

use crate::dom::Element;
use crate::layout::replaced::iframe::IFRAME_DEPTH;
use crate::render::{RenderOpts, blocks, styled_div};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

pub(crate) fn render(e: &Element, opts: &RenderOpts) -> Option<AnyElement> {
    if !e.tag.eq_ignore_ascii_case("img") || IFRAME_DEPTH.with(|d| d.get()) >= 3 {
        return None;
    }
    let src = e.attr("src")?;
    let path = src
        .strip_prefix("file:///")
        .or_else(|| src.strip_prefix("file://"))
        .unwrap_or(src);
    if !path.to_ascii_lowercase().ends_with(".svg") {
        return None;
    }
    let markup = std::fs::read_to_string(path).ok()?;
    if !markup.contains("<foreignObject") {
        return None;
    }
    let (w, h) = natural_size(&markup)?;
    // Only the natural size is supported: scaling an embedded document to a
    // different box would need the SVG viewBox transform on the whole page.
    let fits = |l: Option<Len>, v: f32| match l {
        None | Some(Len::Auto) => true,
        Some(Len::Px(p)) => (p - v).abs() < 0.01,
        _ => false,
    };
    if !fits(e.style.width, w) || !fits(e.style.height, h) {
        return None;
    }
    let html = format!(
        "<!DOCTYPE html><html><head><style>html,body{{margin:0;padding:0}}</style></head><body>{markup}</body></html>"
    );
    let outer_quirks = crate::style::select::quirks();
    let (nodes, salt) = crate::document::parse_embedded_sized(
        &html,
        crate::BROWSER_CSS,
        (w, h),
        false,
        false,
        None,
    );
    let mut sub = opts.clone();
    sub.viewport = (w, h);
    sub.doc_salt = salt;
    sub.cx.positioning = Default::default();
    IFRAME_DEPTH.with(|d| d.set(d.get() + 1));
    sub.cx.positioning.icb_open();
    let mut kids = blocks(&nodes, &sub.root_style(), &sub);
    kids.extend(sub.cx.positioning.icb_close());
    IFRAME_DEPTH.with(|d| d.set(d.get() - 1));
    crate::style::select::QUIRKS.with(|q| q.set(outer_quirks));
    Some(
        styled_div(e, &opts.cx)
            .w(px(w))
            .h(px(h))
            .overflow_hidden()
            .relative()
            .flex_shrink_0()
            .children(kids)
            .into_any_element(),
    )
}

/// `width`/`height` of the root `<svg>` in CSS pixels (unitless or `px`).
fn natural_size(markup: &str) -> Option<(f32, f32)> {
    let start = markup.find("<svg")?;
    let tag = &markup[start..start + markup[start..].find('>')?];
    let attr = |name: &str| -> Option<f32> {
        let at = tag.find(&format!(" {name}="))? + name.len() + 2;
        let rest = &tag[at..];
        let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let value = &rest[1..1 + rest[1..].find(quote)?];
        let value = value.trim();
        value
            .strip_suffix("px")
            .unwrap_or(value)
            .trim()
            .parse()
            .ok()
    };
    Some((attr("width")?, attr("height")?)).filter(|(w, h)| *w > 0.0 && *h > 0.0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn natural_size_reads_root_attributes() {
        let m = "<?xml version='1.0'?><svg xmlns='http://www.w3.org/2000/svg' width=\"500\" height='40px'><g/></svg>";
        assert_eq!(super::natural_size(m), Some((500.0, 40.0)));
        assert_eq!(super::natural_size("<svg width='50%' height='10'/>"), None);
    }
}
