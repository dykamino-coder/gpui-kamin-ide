//! Проверки CSS consumers с подменённым provider: backend не участвует.

use crate::document::state::DocState;
use crate::font_facts::{FontFactsError, FontFactsProvider, FontMetric, FontRegistrySnapshot};
use crate::font_facts::{FontMetricFacts, FontRequest, NormalLineFacts};
use crate::style::computed::Computed;
use crate::style::font_metrics::*;
use crate::style::values::value::Len;
use std::rc::Rc;

struct Facts(f32);
impl FontFactsProvider for Facts {
    fn metrics(
        &self,
        r: FontRequest<'_>,
        _: &FontRegistrySnapshot,
    ) -> Result<FontMetricFacts, FontFactsError> {
        let m = |k| Some(FontMetric::backend_resolved(r.size_px * k * self.0));
        Ok(FontMetricFacts {
            ascent: m(0.8),
            descent: m(0.2),
            cap_height: m(0.7),
            x_height: m(0.4),
            zero_advance: m(0.25),
            ideographic_advance: m(1.5),
            alphabetic_baseline: m(0.05),
            math_script_scale: Some((0.8, 0.6)),
        })
    }
    fn normal_line(
        &self,
        r: FontRequest<'_>,
        _: &FontRegistrySnapshot,
    ) -> Result<NormalLineFacts, FontFactsError> {
        Ok(NormalLineFacts {
            height: Some(FontMetric::backend_resolved(r.size_px * self.0)),
            ..Default::default()
        })
    }
}

fn document(scale: f32) -> Rc<DocState> {
    let doc = DocState::new(Default::default());
    crate::text::metrics::install_probe(|_, _| panic!("CSS bypassed provider"), doc.values());
    doc.install_font_facts(Some(Rc::new(Facts(scale))));
    doc
}

#[test]
fn css_font_lengths_and_roots_follow_each_document_provider() {
    use crate::style::values::value::{
        calc_pct_px, root_line_px, set_root_family, set_root_metrics,
    };
    let a = document(1.0);
    let b = document(2.0);
    for doc in [&a, &b, &a, &b] {
        let vcx = doc.values();
        let factor = if Rc::ptr_eq(doc, &a) { 1.0 } else { 2.0 };
        let mut c = Computed::default();
        c.apply_one("font-size", "20px", vcx);
        c.apply_one("width", "calc(1ex + 1ch + 1ic + 1cap)", vcx);
        c.resolve_em(16.0, vcx);
        assert_eq!(c.width, Some(Len::Px(57.0 * factor)));
        assert_eq!(c.computed_font_px("", 20.0, vcx), Some(20.0));
        c.apply_one("font-size", "2ch", vcx);
        assert_eq!(c.computed_font_px("", 20.0, vcx), Some(10.0 * factor));
        set_root_family(doc, "root");
        set_root_metrics(doc, 20.0, normal_height("root", 20.0, vcx));
        for (unit, px) in [
            ("rex", 8.0),
            ("rch", 5.0),
            ("ric", 30.0),
            ("rcap", 14.0),
            ("rlh", 20.0),
        ] {
            assert_eq!(
                calc_pct_px(&format!("calc(1{unit})"), vcx).unwrap().1,
                px * factor
            );
        }
        assert_eq!(root_line_px(vcx), 20.0 * factor);
    }
}

#[test]
fn trim_and_math_consume_provider_facts_without_backend_queries() {
    use crate::style::computed::TextEdge;
    let doc = document(1.0);
    let vcx = doc.values();
    let mut c = Computed::default();
    c.font_size = Some(Len::Px(20.0));
    c.line_height = Some(Len::Px(30.0));
    c.text_box_over = TextEdge::Ex;
    c.text_box_under = TextEdge::Alphabetic;
    let extents = vertical("", 20.0, vcx);
    assert_eq!(
        crate::text::text_box::edge_inset(&c, "", 20.0, extents, true, vcx),
        8.0
    );
    assert_eq!(
        crate::text::text_box::edge_inset(&c, "", 20.0, extents, false, vcx),
        5.0
    );
    // Сама формула trim прежняя: полулидинг + расстояние до края.
    let mut child = Computed::default();
    child.math_font_size = true;
    child.apply_one("math-depth", "2", vcx);
    let child = crate::style::cascade::inherit::inherit(
        &crate::style::computed::BoxStyle::from(c),
        &child,
        vcx,
    );
    assert_eq!(child.font_size, Some(Len::Px(12.0)));
}

#[test]
fn absent_metrics_and_real_zero_have_distinct_css_fallbacks() {
    struct Missing;
    impl FontFactsProvider for Missing {
        fn metrics(
            &self,
            _: FontRequest<'_>,
            _: &FontRegistrySnapshot,
        ) -> Result<FontMetricFacts, FontFactsError> {
            Ok(FontMetricFacts {
                zero_advance: Some(FontMetric::backend_resolved(0.0)),
                ..Default::default()
            })
        }
        fn normal_line(
            &self,
            _: FontRequest<'_>,
            _: &FontRegistrySnapshot,
        ) -> Result<NormalLineFacts, FontFactsError> {
            Ok(NormalLineFacts::default())
        }
    }
    let doc = document(1.0);
    doc.install_font_facts(Some(Rc::new(Missing)));
    assert_eq!(horizontal("", 20.0, doc.values()), (0.0, 10.0));
    assert_eq!(ideographic("", 20.0, doc.values()), 20.0);
    assert_eq!(normal_height("", 20.0, doc.values()), 24.0);
    assert_eq!(horizontal("", 0.0, doc.values()), (0.0, 0.0));
}
