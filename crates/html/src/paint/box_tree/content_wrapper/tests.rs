//! Intrinsic wrapper eligibility and keyword preservation.

use super::*;
#[test]
fn intrinsic_wrapper_preserves_keyword_without_duplicating_fit_argument() {
    let mut style = Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                width: Some(Len::FitContent),
                ..Default::default()
            },
        ),
        ..Computed::default()
    };
    assert_eq!(
        wrapper_width_keyword(&style),
        Some(CssSizingKeyword::FitContent)
    );
    for argument in [Len::Px(50.0), Len::Pct(0.5)] {
        style.fit_arg[0] = Some(argument);
        assert_eq!(wrapper_width_keyword(&style), None);
    }
    style.fit_arg[0] = None;
    style.width = Some(Len::MinContent);
    assert_eq!(
        wrapper_width_keyword(&style),
        Some(CssSizingKeyword::MinContent)
    );
    style.width = Some(Len::MaxContent);
    assert_eq!(
        wrapper_width_keyword(&style),
        Some(CssSizingKeyword::MaxContent)
    );
    style.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
        true,
        style.wm().block_rl(),
        style.wm().sideways(),
    ));
    assert_eq!(wrapper_width_keyword(&style), None);
}

#[test]
fn ordinary_and_out_of_flow_sizes_do_not_need_an_intrinsic_wrapper() {
    let mut c = Computed::default();
    for size in [
        None,
        Some(Len::Auto),
        Some(Len::Px(80.0)),
        Some(Len::Pct(0.5)),
    ] {
        c.width = size;
        c.height = size;
        assert!(wrapper_tracks(&c).is_none());
    }
    c.width = Some(Len::MinContent);
    for position in [
        crate::style::computed::Position::Absolute,
        crate::style::computed::Position::Fixed,
    ] {
        c.position = Some(position);
        assert!(wrapper_tracks(&c).is_none());
    }
}

#[test]
fn intrinsic_wrapper_eligibility_respects_each_physical_containment_axis() {
    let mut c = Computed::default();
    for size in [Len::MinContent, Len::MaxContent, Len::FitContent] {
        c.width = Some(size);
        c.height = None;
        let (col, row) = wrapper_tracks(&c).unwrap();
        assert!(col.is_some() && row.is_none());
        c.height = Some(size);
        c.contain_inline_size = Some(true);
        let (col, row) = wrapper_tracks(&c).unwrap();
        assert!(col.is_none() && row.is_some());
        c.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
            true, true, false,
        ));
        let (col, row) = wrapper_tracks(&c).unwrap();
        assert!(col.is_some() && row.is_none());
        c.contain_size = Some(true);
        assert!(wrapper_tracks(&c).is_none());
        c.contain_size = None;
        c.contain_inline_size = None;
        c.writing_mode = None;
    }
}
