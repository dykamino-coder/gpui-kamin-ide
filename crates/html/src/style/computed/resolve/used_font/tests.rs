//! Computed/used контракт: наследование, метрики лица, from-font и нулевой кегль.

use super::*;
use crate::font_facts::{
    FontFactsError, FontFactsProvider, FontMetric, FontRegistrySnapshot, FontRequest,
};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::BoxStyle;
use std::rc::Rc;

struct Faces;
impl FontFactsProvider for Faces {
    fn normal_line(
        &self,
        _: FontRequest<'_>,
        _: &FontRegistrySnapshot,
    ) -> Result<crate::font_facts::NormalLineFacts, FontFactsError> {
        Ok(Default::default())
    }
    fn metrics(
        &self,
        request: FontRequest<'_>,
        _: &FontRegistrySnapshot,
    ) -> Result<FontMetricFacts, FontFactsError> {
        if request.family_list == "missing" {
            return Ok(FontMetricFacts::default());
        }
        let ratio = if request.family_list == "wide" {
            0.8
        } else {
            0.4
        };
        let metric = |v| Some(FontMetric::backend_resolved(v * request.size_px));
        Ok(FontMetricFacts {
            x_height: metric(ratio),
            zero_advance: metric(ratio * 1.5),
            cap_height: metric(0.9),
            ..Default::default()
        })
    }
}

fn styled(css: &str, vcx: ValueCx<'_>) -> Computed {
    let mut style = Computed::default();
    style.apply_decls(&crate::style::css::parse_decls(css), vcx);
    style
}
fn doc() -> Rc<crate::document::state::DocState> {
    let doc = crate::document::state::DocState::new(Default::default());
    doc.install_font_facts(Some(Rc::new(Faces)));
    doc
}

#[test]
fn font_size_adjust_keeps_computed_em_and_numeric_line_height() {
    let doc = doc();
    let vcx = doc.values();
    let parent = inherit(
        &BoxStyle::default(),
        &styled(
            "font-family:narrow;font-size:20px;font-size-adjust:0.8;line-height:1.5;width:1em;height:1ex",
            vcx,
        ),
        vcx,
    );
    assert_eq!(parent.font_size, Some(Len::Px(20.0)));
    assert_eq!(parent.width, Some(Len::Px(20.0)));
    assert_eq!(parent.height, Some(Len::Px(16.0)));
    assert_eq!(parent.used_font(16.0, vcx).size_px, 40.0);
    assert_eq!(parent.numeric_line_px(16.0), Some(30.0));
    let child = inherit(
        &parent,
        &styled("font-size:2em;width:1em;height:1ch", vcx),
        vcx,
    );
    assert_eq!(child.font_size, Some(Len::Px(40.0)));
    assert_eq!(child.width, Some(Len::Px(40.0)));
    assert_eq!(child.height, Some(Len::Px(48.0)));
    assert_eq!(child.numeric_line_px(16.0), Some(60.0));
    let ex_child = inherit(&parent, &styled("font-family:wide;font-size:2ex", vcx), vcx);
    assert_eq!(ex_child.font_size, Some(Len::Px(32.0)));
    let calc_child = inherit(&parent, &styled("font-size:calc(1em + 2ex)", vcx), vcx);
    assert_eq!(calc_child.font_size, Some(Len::Px(52.0)));
}

#[test]
fn font_size_adjust_from_font_is_numeric_before_inheriting_another_face() {
    let doc = doc();
    let vcx = doc.values();
    let parent = inherit(
        &BoxStyle::default(),
        &styled(
            "font-family:narrow;font-size:20px;font-size-adjust:from-font",
            vcx,
        ),
        vcx,
    );
    assert_eq!(parent.font_size_adjust, Some((0, 0.4)));
    assert_eq!(parent.used_font(16.0, vcx).size_px, 20.0);
    let child = inherit(&parent, &styled("font-family:wide", vcx), vcx);
    assert_eq!(child.font_size_adjust, parent.font_size_adjust);
    assert_eq!(child.font_size, Some(Len::Px(20.0)));
    assert_eq!(child.used_font(16.0, vcx).size_px, 10.0);
    let missing = inherit(
        &parent,
        &styled("font-family:missing;font-size-adjust:from-font", vcx),
        vcx,
    );
    assert_eq!(missing.used_font(16.0, vcx).size_px, 20.0);
    assert_eq!(missing.font_size_adjust, Some((u8::MAX, 0.0)));
    let vertical = inherit(
        &parent,
        &styled("font-size-adjust:ic-height from-font", vcx),
        vcx,
    );
    assert_eq!(vertical.font_size_adjust, Some((u8::MAX, 0.0)));
    assert_eq!(vertical.used_font(16.0, vcx).size_px, 20.0);
}

#[test]
fn font_size_adjust_zero_used_size_and_overrides_follow_explicit_order() {
    let doc = doc();
    let vcx = doc.values();
    let mut c = styled(
        "font-family:narrow;font-size:0px;line-height:2;font-size-adjust:0.8",
        vcx,
    );
    // R11 каскадного clamp отдельно: used-контракт сам ноль не меняет.
    c.resolve_em(16.0, vcx);
    assert_eq!(c.used_font(16.0, vcx).size_px, 0.0);
    assert_eq!(c.numeric_line_px(16.0), Some(0.0));
    for metric in ["ic-width", "ic-height"] {
        let c = inherit(
            &BoxStyle::default(),
            &styled(
                &format!(
                    "font-family:missing;font-size:20px;line-height:1.5;font-size-adjust:{metric} 2"
                ),
                vcx,
            ),
            vcx,
        );
        assert_eq!(c.used_font(16.0, vcx).size_px, 40.0);
        assert_eq!(c.numeric_line_px(16.0), Some(30.0));
    }
    let used = UsedFont::new(None, 20.0, 2.0, Some((0.5, 0.8)));
    assert_eq!(
        UsedFont::override_metrics(used.size_px, [20.0, 5.0, 0.0], [Some(1.0), None, Some(0.2)]),
        [25.0, 5.0, 5.0]
    );
}

#[test]
fn font_size_adjust_root_rem_and_rex_have_separate_bases() {
    let doc = doc();
    let vcx = doc.values();
    let root = inherit(
        &BoxStyle::default(),
        &styled(
            "font-family:narrow;font-size:20px;font-size-adjust:0.8",
            vcx,
        ),
        vcx,
    );
    crate::style::values::value::set_root_metrics(&doc, 20.0, 30.0);
    let mut request = root.font_request("narrow", root.used_font(16.0, vcx).size_px);
    request.metric_query = FontMetricQuery::Adjustment;
    doc.root.borrow_mut().resolve_font_units(request, vcx);
    assert_eq!(
        crate::style::values::value::calc_pct_px("calc(1rem + 1rex)", vcx),
        Some((0.0, 36.0))
    );
}
