//! Замещаемое содержимое может быть пустым, не стирая свою CSS-коробку: размер пустого
//! `<iframe>` по умолчанию объекта (CSS 2.1 §10.3.2, §10.6.2) с учётом quirks-долей и
//! `contain: size`, `object-position` у `Img` и замещаемый `<svg>` внутри стилевого `div`
//! при ненулевой рамке или отбивке (CSS 2.1 §10.3.4), чтобы фон и рамка рисовались.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::{Len, px_lossy_zero};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

/// CSS 2.1 sections 10.3.2 and 10.6.2: an empty browsing context is still
/// replaced content; its auto dimensions use the default object size.
pub(crate) fn empty_iframe_size(
    e: &Element,
    inherited: &crate::style::computed::BoxStyle,
    viewport: (f32, f32),
    vcx: ValueCx<'_>,
) -> Element {
    let mut copy = crate::layout::replaced::image::pct_height_to_px(e, inherited);
    // Quirks percentage heights can skip auto-height ancestors (Quirks section 3.5).
    // Resolve that inherited basis before treating an indefinite percentage as auto.
    let merged = crate::style::cascade::materialize::box_style(inherited, &copy.style, vcx);
    copy.style.used_mut().height = merged.height;
    copy.style.used_mut().cb_height_def_hint = merged.cb_height_def;
    if copy.style.computed().has_viewport_lengths(vcx) {
        copy.style.used_mut().resolve_viewport(viewport, vcx);
    }
    if vcx.doc.quirks()
        && !matches!(
            copy.style.computed().position,
            Some(
                crate::style::computed::Position::Absolute
                    | crate::style::computed::Position::Fixed
            )
        )
        && let Some(Len::Pct(k)) = copy.style.computed().height
    {
        copy.style.used_mut().height =
            Some(Len::Px(k * inherited.quirk_pct_base.unwrap_or(viewport.1)));
    }
    // Containment suppresses natural dimensions (CSS Containment 2 §3.1),
    // while retaining authored dimensions, ratios and intrinsic overrides.
    if copy.style.computed().contain_size == Some(true) {
        return copy;
    }
    let ratio = copy
        .style
        .computed()
        .aspect_ratio
        .is_some_and(|r| r.is_finite() && r > 0.0);
    let auto_width = matches!(copy.style.computed().width, None | Some(Len::Auto));
    let auto_height = matches!(copy.style.computed().height, None | Some(Len::Auto))
        || (matches!(copy.style.computed().height, Some(Len::Pct(_)))
            && !inherited.cb_height_def
            && !matches!(
                copy.style.computed().position,
                Some(
                    crate::style::computed::Position::Absolute
                        | crate::style::computed::Position::Fixed
                )
            ));
    if auto_width && (!ratio || auto_height) {
        let width = if copy.style.computed().contains_width() {
            copy.style.computed().contain_intrinsic.0.unwrap_or(0.0)
        } else {
            300.0
        };
        copy.style.used_mut().width = Some(Len::Px(width));
    }
    if auto_height {
        copy.style.used_mut().height = if ratio {
            None
        } else {
            let height = if copy.style.computed().contains_height() {
                copy.style.computed().contain_intrinsic.1.unwrap_or(0.0)
            } else {
                150.0
            };
            Some(Len::Px(height))
        };
    }
    if ratio
        && auto_height
        && let (Some(Len::Px(w)), Some(r)) = (
            copy.style.computed().width,
            copy.style.computed().aspect_ratio,
        )
    {
        copy.style.used_mut().height = Some(Len::Px(w / r));
    }
    copy
}

pub(crate) fn empty_iframe(
    e: &Element,
    inherited: &crate::style::computed::BoxStyle,
    opts: &crate::layout::options::RenderOpts,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let mut copy = empty_iframe_size(e, inherited, opts.viewport, vcx);
    if copy.style.computed().contain_size == Some(true) {
        copy.attrs.retain(|(name, _)| name != "src");
        return opts.cx.build.replaced(&copy, None, &opts.cx);
    }
    opts.cx
        .build
        .styled_box(
            &copy,
            &crate::style::computed::BoxStyle::shared(copy.style.computed()),
            &opts.cx,
        )
        .flex_shrink_0()
        .into_any_element()
}

pub(crate) fn default_iframe(e: &Element) -> bool {
    e.tag == "iframe"
        && e.attr("src").is_none_or(|src| src.trim().is_empty())
        && e.style.computed().contain_size != Some(true)
}

/// Замещаемый `<svg>` с учётом его CSS-коробки (CSS 2.1 §10.3.4): растр —
/// содержимое, а при ненулевой рамке или отбивке (`svg_has_box`) — внутри
/// стилевого `div` размером border-box, фон красит он. Прежде растр шёл
/// голым и рамка не рисовалась вовсе (border-shape-clips-background-ref,
/// mask-image-svg-child-will-change: маска ложится на коробку 200×200 с
/// рамкой 50). Без рамки и отбивки — голый растр, путь прежний.
/// `None` — рисунок не разобрался.
pub(crate) fn svg_replaced(
    e: &Element,
    sized: &Element,
    merged: &crate::style::computed::BoxStyle,
    opts: &crate::layout::options::RenderOpts,
) -> Option<AnyElement> {
    let boxed = svg_has_box(merged);
    let inner;
    let sized = if boxed {
        let mut copy = sized.clone();
        copy.style.used_mut().background = None;
        inner = copy;
        &inner
    } else {
        sized
    };
    let raster = crate::svg::element_with_html(sized, opts);
    if !boxed {
        return raster;
    }
    let px_of = px_lossy_zero;
    let (w, h) = crate::svg::size::size_of(sized);
    // Empty replaced content still has its CSS padding, border and background.
    if raster.is_none() && w > 0.0 && h > 0.0 {
        return None;
    }
    let b = merged.borders();
    let bw = w
        + px_of(b.left)
        + px_of(b.right)
        + px_of(merged.padding.left)
        + px_of(merged.padding.right);
    let bh = h
        + px_of(b.top)
        + px_of(b.bottom)
        + px_of(merged.padding.top)
        + px_of(merged.padding.bottom);
    Some(
        opts.cx
            .build
            .styled_box(e, merged, &opts.cx)
            .w(px(bw))
            .h(px(bh))
            .flex_shrink_0()
            .children(raster)
            .into_any_element(),
    )
}

/// У `<svg>` есть своя CSS-коробка — ненулевая рамка или отбивка
/// (см. ветку `"svg"` в `element`): тогда растр кладётся в стилевой `div`.
pub(super) fn svg_has_box(c: &Computed) -> bool {
    let nz = |l: Option<Len>| matches!(l, Some(Len::Px(v)) if v > 0.0);
    let b = c.borders();
    nz(b.top)
        || nz(b.right)
        || nz(b.bottom)
        || nz(b.left)
        || nz(c.padding.top)
        || nz(c.padding.right)
        || nz(c.padding.bottom)
        || nz(c.padding.left)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_floated_iframe_keeps_replaced_dimensions() {
        let doc = crate::document::state::DocState::new(Default::default());
        let mut frame =
            Element::synthetic("iframe", Computed::default(), Default::default(), vec![]);
        frame.style.used_mut().floated_box = true;
        frame.style.used_mut().flow_root = Some(true);
        for (height, expected) in [(None, 150.0), (Some(Len::Pct(0.5)), 96.0)] {
            frame.style.used_mut().height = height;
            let mut parent = crate::style::computed::BoxStyle::from(Computed {
                height: Some(Len::Px(192.0)),
                ..Default::default()
            });
            parent.cb_height_def = true;
            let sized = empty_iframe_size(&frame, &parent, (800.0, 600.0), doc.values());
            assert_eq!(sized.style.computed().width, Some(Len::Px(300.0)));
            assert_eq!(sized.style.computed().height, Some(Len::Px(expected)));
        }
    }
}
