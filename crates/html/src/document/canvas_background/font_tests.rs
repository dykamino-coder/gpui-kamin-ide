//! Длины фона холста используют вычисленный шрифт body и факты документа.

use super::*;
use crate::dom::test_support::find;
use crate::font_facts::{
    FontFactsError, FontFactsProvider, FontMetric, FontMetricFacts, FontRegistrySnapshot,
    FontRequest, NormalLineFacts,
};
use crate::style::canvas_background::CanvasBackgroundSource;
use crate::style::computed::BgSize;
use std::rc::Rc;

struct Facts(f32);

impl FontFactsProvider for Facts {
    fn metrics(
        &self,
        request: FontRequest<'_>,
        _: &FontRegistrySnapshot,
    ) -> Result<FontMetricFacts, FontFactsError> {
        Ok(FontMetricFacts {
            x_height: Some(FontMetric::backend_resolved(request.size_px * self.0)),
            zero_advance: Some(FontMetric::backend_resolved(request.size_px * 0.75)),
            ..Default::default()
        })
    }

    fn normal_line(
        &self,
        request: FontRequest<'_>,
        _: &FontRegistrySnapshot,
    ) -> Result<NormalLineFacts, FontFactsError> {
        Ok(NormalLineFacts {
            height: Some(FontMetric::backend_resolved(request.size_px)),
            ..Default::default()
        })
    }
}

#[test]
fn donor_font_lengths_use_body_computed_font_and_document_facts() {
    for x_height in [0.8, 0.4] {
        for image in ["url(a)", "url(a), url(b)"] {
            let parsed = crate::dom::parse(
                &format!(
                    "<style>html {{ font:20px/1 Ahem }} body {{ font-size:2.5ex; background-image:{image}; background-position:1em 6.25ex, 2ch 50%; background-size:2em 3ch, 4ex 50% }}</style>"
                ),
                "",
            );
            let vcx = parsed.values();
            vcx.doc.install_font_facts(Some(Rc::new(Facts(x_height))));
            crate::text::metrics::install_probe(|_, _| panic!("bypassed font facts"), vcx);
            let nodes = mark_canvas_background(parsed.nodes.clone(), vcx);
            let root = find(&nodes, &|e| e.tag == "html").unwrap();
            let body = find(&nodes, &|e| e.tag == "body").unwrap();
            let font = 20.0 * x_height * 2.5;
            let source = root.style.used().canvas_background.unwrap();
            let CanvasBackgroundSource::Donor(background) = &source else {
                panic!("expected body donor");
            };
            assert_eq!(background.top.position.x, Some(Len::Px(font)));
            assert_eq!(
                background.top.position.y,
                Some(Len::Px(6.25 * font * x_height))
            );
            assert_eq!(
                background.top.size,
                BgSize::Fixed(Some(Len::Px(2.0 * font)), Some(Len::Px(2.25 * font)))
            );
            if let Some(layers) = &background.layers {
                assert_eq!(layers[1].position.x, Some(Len::Px(1.5 * font)));
                assert_eq!(layers[1].position.y, Some(Len::Pct(0.5)));
                assert_eq!(
                    layers[1].size,
                    BgSize::Fixed(Some(Len::Px(4.0 * font * x_height)), Some(Len::Pct(0.5)))
                );
            }
            assert_eq!(body.style.cascaded().bg_pos.x, Some(Len::Em(1.0)));
            assert!(!body.style.used().background_paint.paints());
            crate::style::cascade::materialize::materialize(
                &nodes,
                &Rc::new(Computed::default()),
                vcx,
            );
            assert_eq!(root.style.computed().font_size, Some(Len::Px(20.0)));
            assert_eq!(body.style.computed().font_size, Some(Len::Px(font)));
            assert_eq!(root.style.computed().bg_pos.y, None);
        }
    }
}
