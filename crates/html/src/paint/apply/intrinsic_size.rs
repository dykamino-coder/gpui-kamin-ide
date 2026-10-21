//! Применение к GPUI: ключевые слова внутреннего размера (`min-content`,
//! `max-content`, `fit-content`, `fit-content(N)`, `stretch`) доносятся до родной
//! раскладки как `CssSizingKeyword`, а не теряются на границе. Здесь же
//! решается, нужен ли коробке родной контракт content-box: доля и функциональный
//! `fit-content` прибавляют разрешённые края к используемому размеру
//! (CSS 2.1 §10.2, §10.5). При `contain: size` ключевые слова содержимое не меряют.
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::CssSizingKeyword;

/// Percentages and functional fit need the native content-box contract: their
/// used size must add the resolved edges (CSS 2.1 sections 10.2 and 10.5).
pub(crate) fn native_content_box(style: &Computed) -> bool {
    style.border_box != Some(true)
        && ((style.width == Some(Len::FitContent)
            && matches!(style.fit_arg[0], Some(Len::Px(_) | Len::Pct(_))))
            || [
                style.width,
                style.height,
                style.min_width,
                style.max_width,
                style.min_height,
                style.max_height,
            ]
            .into_iter()
            .zip(style.stretch_size)
            .any(|(size, stretch)| !stretch && matches!(size, Some(Len::Pct(_))))
            || [
                style.padding.left,
                style.padding.right,
                style.padding.top,
                style.padding.bottom,
            ]
            .into_iter()
            .any(|side| matches!(side, Some(Len::Pct(_)))))
}

pub(crate) fn keywords(style: &Computed) -> [Option<CssSizingKeyword>; 2] {
    let keyword = |value, argument| match value {
        Some(Len::MinContent) => Some(CssSizingKeyword::MinContent),
        Some(Len::MaxContent) => Some(CssSizingKeyword::MaxContent),
        Some(Len::FitContent) => Some(match argument {
            Some(Len::Px(value)) => CssSizingKeyword::FitContentPixels(value),
            Some(Len::Pct(value)) => CssSizingKeyword::FitContentPercent(value),
            _ => CssSizingKeyword::FitContent,
        }),
        _ => None,
    };
    // css-sizing-4 §4.1: `stretch` is kept as `Len::Pct(1.0)` for the
    // percentage paths, but the native layout must see the keyword: it fills
    // the margin box (and, when positioned, the inset-modified containing
    // block), which no percentage expresses. It does not measure content, so
    // containment does not affect it.
    let stretch = |axis: usize| style.stretch_size[axis].then_some(CssSizingKeyword::Stretch);
    // CSS Containment 2 §3.1: apply_box already supplied the empty-box
    // intrinsic size. Native keywords must not measure the real children again.
    [
        stretch(0).or(if style.contains_width() {
            None
        } else {
            keyword(style.width, style.fit_arg[0])
        }),
        stretch(1).or(if style.contains_height() {
            None
        } else {
            keyword(style.height, None)
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Styled, div, px};
    #[test]
    fn functional_fit_uses_one_box_model_for_keywords_and_numeric_axes() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed {
            width: Some(Len::FitContent),
            height: Some(Len::Px(60.0)),
            ..Computed::default()
        };
        c.border_visible = [Some(true); 4];
        c.border_width.left = Some(Len::Px(3.0));
        c.border_width.right = Some(Len::Px(3.0));
        for argument in [Len::Px(394.0), Len::Pct(0.5)] {
            c.fit_arg[0] = Some(argument);
            assert!(native_content_box(&c));
            let mut d = super::super::apply(div(), &c, vcx);
            assert_eq!(d.style().content_box, Some(true));
            assert_eq!(d.style().size.height, Some(px(60.0).into()));
        }
        c.border_box = Some(true);
        assert!(!native_content_box(&c));
        c.border_box = None;
        c.fit_arg[0] = None;
        assert!(!native_content_box(&c));
        c.width = Some(Len::Px(100.0));
        c.padding.left = Some(Len::Pct(0.1));
        assert!(native_content_box(&c));
    }
}
