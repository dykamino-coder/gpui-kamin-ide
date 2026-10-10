//! Adapter dimensions preserve keyword identity and scale only absolute lengths.
use super::ToTaffy;
use crate::{CssSizingKeyword, Style, px};

#[test]
fn native_keywords_and_ratio_basis_survive_style_conversion() {
    let style = Style {
        sizing_keywords: [
            Some(CssSizingKeyword::MinContent),
            Some(CssSizingKeyword::FitContent),
        ],
        aspect_ratio_preferred_size: [None, Some(50.0)],
        ..Style::default()
    };
    let native: taffy::Style = style.to_taffy(px(16.0), 1.25);
    assert_eq!(native.size.width, taffy::Dimension::min_content());
    assert_eq!(native.size.height, taffy::Dimension::fit_content());
    assert_eq!(native.aspect_ratio_preferred_size.height, Some(62.5));
}

#[test]
fn fit_content_arguments_keep_length_and_percentage_units() {
    let style = Style {
        sizing_keywords: [
            Some(CssSizingKeyword::FitContentPixels(40.0)),
            Some(CssSizingKeyword::FitContentPercent(0.5)),
        ],
        ..Style::default()
    };
    let native: taffy::Style = style.to_taffy(px(16.0), 1.25);
    assert_eq!(native.size.width, taffy::Dimension::fit_content_px(50.0));
    assert_eq!(
        native.size.height,
        taffy::Dimension::fit_content_percent(0.5)
    );
}
