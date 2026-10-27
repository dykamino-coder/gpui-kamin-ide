//! SVG-картинка, чьё содержимое требует HTML-движка (`<foreignObject>`).
//!
//! `<img>`, указывающий на SVG-файл, — независимый документ в безопасном статическом
//! режиме (SVG 2 §conform-referencing-modes, HTML §4.8.4.3): скрипты не выполняются, но
//! `<foreignObject>` всё равно рисует свой HTML. resvg пропускает `<foreignObject>`
//! целиком, поэтому такой файл раскладывается как самостоятельный документ `.svg`
//! (корень SVG во вложенном документе с природным размером картинки, как страница
//! `SVGImage` в Blink), а не растрируется. Картинки без `<foreignObject>` остаются
//! на растровом пути.

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use crate::tolerance::COORD_EPS;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

pub(crate) fn render(e: &Element, opts: &RenderOpts) -> Option<AnyElement> {
    if !e.tag.eq_ignore_ascii_case("img") || opts.cx.contracts.frame_depth >= 3 {
        return None;
    }
    let src = e.attr("src")?;
    let (crate::resources::images::types::DecodedImage::Vector(vector), _) =
        opts.cx.vcx().doc.images.borrow_mut().resolve_image(
            src,
            crate::resources::images::types::DecodeOptions::default(),
        )?
    else {
        return None;
    };
    let markup = &vector.markup;
    if !markup.contains("<foreignObject") {
        return None;
    }
    let (w, h) = natural_size(&markup)?;
    // Only the natural size is supported: scaling an embedded document to a
    // different box would need the SVG viewBox transform on the whole page.
    let fits = |l: Option<Len>, v: f32| match l {
        None | Some(Len::Auto) => true,
        Some(Len::Px(p)) => (p - v).abs() < COORD_EPS,
        _ => false,
    };
    if !fits(e.style.used().width, w) || !fits(e.style.used().height, h) {
        return None;
    }
    let html = format!(
        "<!DOCTYPE html><html><head><style>html,body{{margin:0;padding:0}}</style></head><body>{markup}</body></html>"
    );
    let embedded = match crate::document::parse_embedded_sized(
        &html,
        crate::BROWSER_CSS,
        (w, h),
        opts.cx.doc().media().print,
        false,
        false,
        None,
        &opts.cx.doc().metrics.font_context(),
        opts.cx.doc().input_context(),
    ) {
        Ok(embedded) => embedded,
        Err(failure) => {
            return Some(super::iframe::rejected(failure, opts));
        }
    };
    let kids = super::iframe::build_embedded(embedded, (w, h), opts, |nodes| nodes);
    Some(
        opts.cx
            .contracts
            .build
            .styled_box(e, &BoxStyle::shared(e.style.used()), &opts.cx)
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
