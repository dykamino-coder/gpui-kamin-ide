//! Размер картинки под обособлением размера (contain: size): стороны содержимого из
//! заданных размеров и `contain-intrinsic-size`, заявленное соотношение и пределы.

use crate::style::computed::Computed;
use crate::style::values::value::{Len, px_lossy_zero};

/// Размер обособленной картинки.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ContainedSize {
    /// Стороны содержимого.
    pub(crate) content: (f32, f32),
    /// Ширина и высота держателя (border-box) по обособленной оси с авто-размером.
    pub(crate) holder: (Option<f32>, Option<f32>),
}

/// `None` — обособления размера нет.
pub(crate) fn contained_size(style: &Computed) -> Option<ContainedSize> {
    if !(style.contains_width() || style.contains_height()) {
        return None;
    }
    let side = px_lossy_zero;
    let b = style.borders();
    let pad_x = side(style.padding.left) + side(style.padding.right) + side(b.left) + side(b.right);
    let pad_y = side(style.padding.top) + side(style.padding.bottom) + side(b.top) + side(b.bottom);
    let used = |explicit: Option<Len>, ci: Option<f32>| match explicit {
        Some(Len::Px(v)) => v,
        _ => ci.unwrap_or(0.0),
    };
    // Обособление снимает ПРИРОДНОЕ соотношение, но не ЗАЯВЛЕННОЕ:
    // css-contain-2 §size containment, «Size containment only suppresses
    // the natural aspect ratio, so properties like 'aspect-ratio' which
    // affect that preferred aspect ratio directly are honored»
    // (Overview.bs:659-662), и пример там же (:737-752):
    // `img{width:100px;aspect-ratio:1/1;contain:size}` = 100×100, а без
    // объявленного соотношения — 100×0. Пара атрибутов `width`/`height`
    // разметки — это `aspect-ratio: auto <ratio>` (HTML Rendering
    // §attributes for embedded content): природная его половина снята,
    // заявленная осталась. Blink делает ровно так —
    // `BlockNode::GetReplacedAspectRatio` (`block_node.cc:1328`):
    // заявленное отдаётся ДО проверки обособления, гейт
    // `ShouldApplyAnySizeContainment` стоит только вокруг природного.
    let ratio = style
        .aspect_ratio
        .filter(|r| r.is_finite() && *r > 0.0)
        .or_else(|| match (style.attr_width, style.attr_height) {
            (Some(Len::Px(aw)), Some(Len::Px(ah))) if aw > 0.0 && ah > 0.0 => Some(aw / ah),
            _ => None,
        });
    let named = |l: Option<Len>| matches!(l, Some(Len::Px(_)));
    let (nw, nh) = (named(style.width), named(style.height));
    let cw = match ratio {
        Some(r) if !nw && nh => used(style.height, style.contain_intrinsic.1) * r,
        _ => used(style.width, style.contain_intrinsic.0),
    };
    let ch = match ratio {
        Some(r) if nw && !nh => used(style.width, style.contain_intrinsic.0) / r,
        _ => used(style.height, style.contain_intrinsic.1),
    };
    // CSS Containment 2 §3.1: lay out content in the final box, including
    // its min/max constraints, rather than the suppressed natural size.
    let clamp = |value: f32, min: Option<Len>, max: Option<Len>, padding: f32| {
        let content = |length| match length {
            Some(Len::Px(v)) => Some(if style.border_box == Some(true) {
                (v - padding).max(0.0)
            } else {
                v
            }),
            _ => None,
        };
        value
            .min(content(max).unwrap_or(f32::INFINITY))
            .max(content(min).unwrap_or(0.0))
    };
    let cw = clamp(cw, style.min_width, style.max_width, pad_x);
    let ch = clamp(ch, style.min_height, style.max_height, pad_y);
    let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    Some(ContainedSize {
        content: (cw, ch),
        holder: (
            (style.contains_width() && auto(style.width)).then_some(cw + pad_x),
            (style.contains_height() && auto(style.height)).then_some(ch + pad_y),
        ),
    })
}
