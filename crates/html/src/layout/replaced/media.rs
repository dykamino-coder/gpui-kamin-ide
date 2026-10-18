//! Sizing of replaced media without available natural dimensions.

use crate::dom::Element;
use crate::layout::replaced::{image, limits::css2_replaced_limits, replaced_used_style};
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::{Len, px_strict};
use gpui::AnyElement;

pub(crate) fn empty(e: &Element) -> bool {
    match e.tag.as_str() {
        "video" => e.attr("poster").is_none(),
        "embed" => e.attr("src").is_none() && e.attr("type").is_some(),
        "audio" => e.style.contains_width() || e.style.contains_height(),
        "object" => e.attr("data").is_none() && e.style.contain_size == Some(true),
        _ => false,
    }
}

/// HTML 15.4.1 and CSS Images 3 default sizing: fallback dimensions are
/// separate from a natural aspect ratio and disappear under size containment.
/// `avail_w` — доступная ширина блока (`LayoutCx::avail_w`) для долей.
pub(crate) fn resolve(
    e: &Element,
    inherited: &Computed,
    viewport: (f32, f32),
    avail_w: Option<f32>,
) -> Option<Element> {
    if !empty(e) {
        return None;
    }
    let normalized = replaced_used_style::normalize(e, avail_w);
    let e = normalized.as_ref().unwrap_or(e);
    let mut copy = image::pct_height_to_px(e, inherited);
    copy.style.resolve_viewport(viewport);
    // A media resource is not an image source. Its unavailable natural size
    // must not trigger image decoding or a broken-image text placeholder.
    copy.attrs.retain(|(name, _)| name != "src");
    if copy.style.contain_size == Some(true) {
        return Some(copy);
    }
    let s = &mut copy.style;
    let ratio = s
        .aspect_ratio
        .or(s.aspect_ratio_auto)
        .or_else(|| {
            if e.tag != "video" {
                return None;
            }
            match (s.attr_width, s.attr_height) {
                (Some(Len::Px(w)), Some(Len::Px(h))) if w > 0.0 && h > 0.0 => Some(w / h),
                _ => None,
            }
        })
        .filter(|r| r.is_finite() && *r > 0.0);
    let auto = |v| matches!(v, None | Some(Len::Auto));
    let (aw, ah) = (auto(s.width), auto(s.height));
    let px = |v| px_strict(v);
    let clamp = |v: f32, lo: Option<Len>, hi: Option<Len>| {
        v.min(px(hi).unwrap_or(f32::INFINITY))
            .max(px(lo).unwrap_or(0.0))
    };
    let dw = if s.contains_width() {
        s.contain_intrinsic.0.unwrap_or(0.0)
    } else {
        300.0
    };
    let dh = if s.contains_height() {
        s.contain_intrinsic.1.unwrap_or(0.0)
    } else {
        150.0
    };
    match (aw, ah, ratio) {
        (true, true, Some(r)) => {
            let w = dw.min(dh * r);
            let (w, h) = css2_replaced_limits(
                w,
                w / r,
                px(s.min_width),
                px(s.max_width),
                px(s.min_height),
                px(s.max_height),
            );
            s.width = Some(Len::Px(w));
            s.height = Some(Len::Px(h));
        }
        (true, false, Some(r)) => {
            if let Some(h) = px(s.height) {
                s.width = Some(Len::Px(clamp(h * r, s.min_width, s.max_width)));
            }
        }
        (false, true, Some(r)) => {
            if let Some(w) = px(s.width) {
                s.height = Some(Len::Px(clamp(w / r, s.min_height, s.max_height)));
            }
        }
        _ => {
            if aw {
                s.width = Some(Len::Px(dw));
            }
            if ah {
                s.height = Some(Len::Px(dh));
            }
        }
    }
    Some(copy)
}

pub(crate) fn render(e: &Element, opts: &RenderOpts) -> AnyElement {
    image::image(e, &opts.cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video() -> Element {
        crate::layout::table::anon::anon_element("video", vec![])
    }

    #[test]
    fn fallback_dimensions_do_not_imply_a_ratio() {
        let mut e = video();
        e.style.height = Some(Len::Px(40.0));
        let sized = resolve(&e, &Computed::default(), (800.0, 600.0), None).unwrap();
        assert_eq!(sized.style.width, Some(Len::Px(300.0)));
        assert_eq!(sized.style.height, Some(Len::Px(40.0)));
    }

    #[test]
    fn dimension_attributes_transfer_maximum_constraints() {
        let mut e = video();
        e.style.attr_width = Some(Len::Px(100.0));
        e.style.attr_height = Some(Len::Px(100.0));
        e.style.max_width = Some(Len::Px(100.0));
        let sized = resolve(&e, &Computed::default(), (800.0, 600.0), None).unwrap();
        assert_eq!(sized.style.width, Some(Len::Px(100.0)));
        assert_eq!(sized.style.height, Some(Len::Px(100.0)));
    }

    #[test]
    fn size_containment_keeps_default_dimensions_out_of_style() {
        let mut e = video();
        e.style.contain_size = Some(true);
        let sized = resolve(&e, &Computed::default(), (800.0, 600.0), None).unwrap();
        assert_eq!(sized.style.width, None);
        assert_eq!(sized.style.height, None);
    }
}
