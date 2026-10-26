//! Проверки вычисленных и измеренных размеров многоколоночного хоста.

use super::*;
use crate::style::computed::BoxStyle;

#[test]
fn zero_column_width_uses_one_pixel_without_changing_computed_value() {
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800., 600.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    for (css, expected) in [
        ("column-width:0", Len::Px(0.)),
        ("columns:0", Len::Px(0.)),
        ("column-width:0.5px", Len::Px(0.5)),
        ("column-width:0em", Len::Em(0.)),
        ("column-width:0.01em", Len::Em(0.01)),
    ] {
        let doc = crate::document::state::DocState::new(Default::default());
        let mut style = Computed::default();
        style.apply_decls_with_vars(
            &crate::style::css::parse_decls(&format!("width:50px; column-gap:0; {css}")),
            &Default::default(),
            doc.values(),
        );
        assert_eq!(style.column_width, Some(expected), "{css}");
        let e = Element::synthetic("div", style.clone(), Default::default(), vec![]);
        let merged =
            crate::style::cascade::inherit::inherit(&BoxStyle::default(), &style, doc.values());
        let plan = column_plan(&e, &merged, &opts);
        assert_eq!(plan.column_width, Some(Len::Px(1.0)), "{css}");
        assert_eq!(plan.used_count, Some(50), "{css}");
        assert_eq!(style.column_width, Some(expected));
    }
}

#[test]
fn measured_inline_size_controls_count_and_percentage_gap_without_css_writes() {
    let style = Computed {
        width: Some(Len::Pct(0.5)),
        height: Some(Len::Px(100.)),
        column_width: Some(Len::Px(40.)),
        column_gap: Some(Len::Pct(0.1)),
        ..Default::default()
    };
    let e = Element::synthetic("div", style.clone(), Default::default(), vec![]);
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800., 600.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let dimensions = MulticolDimensions::measured(&style, 200., Some(80.));
    let plan = column_plan_in(&e, &style, &opts, dimensions, dimensions);
    assert_eq!(plan.used_gap, 20.);
    assert_eq!(plan.used_count, Some(3));
    assert_eq!(plan.col_inline_size, Some(Len::Px(200.)));
    assert_eq!(e.style.used().width, Some(Len::Pct(0.5)));
    assert_eq!(style.height, Some(Len::Px(100.)));
}
