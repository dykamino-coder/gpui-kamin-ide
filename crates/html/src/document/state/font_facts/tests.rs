//! Проверки внедрения, ревизий и изоляции фактов при чередовании документов.

use super::*;
use crate::font_facts::{
    FontFactsError, FontMetric, FontMetricFacts, FontRequest, NormalLineFacts,
};
use crate::style::values::value::{
    calc_pct_px, root_normal_line_px, set_root_family, set_root_metrics,
};
use crate::text::metrics::{self, FontContext};
use std::cell::Cell;

struct Fake {
    scale: f32,
    missing: bool,
}

impl FontFactsProvider for Fake {
    fn metrics(
        &self,
        request: FontRequest<'_>,
        _: &FontRegistrySnapshot,
    ) -> Result<FontMetricFacts, FontFactsError> {
        let k = if request.family_list == "root-test-wide" {
            1.0
        } else {
            0.5
        };
        let size = request.size_px * self.scale;
        Ok(FontMetricFacts {
            ascent: Some(FontMetric::backend_resolved(size)),
            cap_height: (!self.missing).then(|| FontMetric::backend_resolved(size * 0.75)),
            x_height: (!self.missing).then(|| FontMetric::backend_resolved(size * k * 0.8)),
            zero_advance: Some(FontMetric::backend_resolved(if self.missing {
                0.0
            } else {
                size * k
            })),
            ideographic_advance: (!self.missing).then(|| FontMetric::backend_resolved(size * k)),
            ..FontMetricFacts::default()
        })
    }

    fn normal_line(
        &self,
        request: FontRequest<'_>,
        _: &FontRegistrySnapshot,
    ) -> Result<NormalLineFacts, FontFactsError> {
        Ok(NormalLineFacts {
            height: (!self.missing)
                .then(|| FontMetric::backend_resolved(request.size_px * self.scale)),
            ..NormalLineFacts::default()
        })
    }
}

fn unit(doc: &DocState, name: &str) -> f32 {
    calc_pct_px(&format!("calc(1r{name})"), doc.values())
        .unwrap()
        .1
}

#[test]
fn fake_provider_drives_root_units_and_normal_through_public_bootstrap() {
    let fonts = FontContext::default().with_facts_provider(|| {
        Rc::new(Fake {
            scale: 1.0,
            missing: false,
        })
    });
    let doc = crate::Document::builder("<p>root</p>", "")
        .fonts(&fonts)
        .build()
        .unwrap();
    let doc = &doc.state;
    set_root_family(doc, "root-test-wide");
    set_root_metrics(doc, 20.0, 25.0);
    assert_eq!(
        [
            unit(doc, "cap"),
            unit(doc, "ch"),
            unit(doc, "ex"),
            unit(doc, "ic")
        ],
        [15.0, 20.0, 16.0, 20.0]
    );
    assert_eq!(root_normal_line_px("Face", 20.0, doc.values()), 20.0);
    set_root_metrics(doc, 40.0, 50.0);
    assert_eq!(unit(doc, "ch"), 40.0);
    set_root_family(doc, "root-test-narrow");
    assert_eq!(
        [unit(doc, "ch"), unit(doc, "ex"), unit(doc, "ic")],
        [20.0, 16.0, 20.0]
    );
    assert_eq!(
        calc_pct_px("calc(1rcap + 2rch + 1rex + 1ric)", doc.values()),
        Some((0.0, 106.0))
    );
}

#[test]
fn two_live_documents_keep_facts_across_other_document_bootstrap_reset_and_update() {
    let instances = Rc::new(Cell::new(0));
    let count = instances.clone();
    let fonts = FontContext::default().with_facts_provider(move || {
        count.set(count.get() + 1);
        Rc::new(Fake {
            scale: count.get() as f32,
            missing: false,
        })
    });
    let a = crate::Document::builder("<p>A</p>", "")
        .fonts(&fonts)
        .build()
        .unwrap();
    let snapshot = a.state.font_snapshot();
    let mut b = crate::Document::builder("<p>B</p>", "")
        .fonts(&fonts)
        .build()
        .unwrap();
    for doc in [&a, &b] {
        set_root_metrics(&doc.state, 20.0, 25.0);
    }
    assert_eq!((unit(&a.state, "ch"), unit(&b.state, "ch")), (10.0, 20.0));
    assert_ne!(snapshot.environment, b.state.font_snapshot().environment);
    assert!(b.update("<p>B updated</p>", "").unwrap().changed());
    assert_eq!(instances.get(), 3);
    set_root_metrics(&b.state, 20.0, 25.0);
    assert_eq!(unit(&b.state, "ch"), 30.0);
    // Разбор и повторный bootstrap сбрасывают только реестры/кэши своего документа.
    b.state.reset_font_alternates();
    FontContext::default().install(b.state.values());
    drop(b);
    let fresh = DocState::new(Default::default());
    assert_ne!(snapshot.environment, fresh.font_snapshot().environment);
    assert_eq!(snapshot, a.state.font_snapshot());
    set_root_metrics(&a.state, 20.0, 25.0);
    assert_eq!(unit(&a.state, "ch"), 10.0);
    assert_eq!(root_normal_line_px("Face", 20.0, a.state.values()), 20.0);
}

#[test]
fn resolver_distinguishes_missing_metrics_from_zero_and_uses_ascent_for_cap() {
    let doc = DocState::new(Default::default());
    doc.install_font_facts(Some(Rc::new(Fake {
        scale: 1.0,
        missing: true,
    })));
    set_root_metrics(&doc, 20.0, 25.0);
    assert_eq!(
        [
            unit(&doc, "cap"),
            unit(&doc, "ch"),
            unit(&doc, "ex"),
            unit(&doc, "ic")
        ],
        [20.0, 0.0, 10.0, 20.0]
    );
    assert_eq!(root_normal_line_px("Face", 20.0, doc.values()), 24.0);
}

#[test]
fn backend_rejects_foreign_stale_and_expired_document_snapshots() {
    let a = DocState::new(Default::default());
    let b = DocState::new(Default::default());
    let backend = a.font_provider();
    let request = FontRequest::new("Face", 20.0);
    assert_eq!(
        backend.metrics(request, &b.font_snapshot()),
        Err(FontFactsError::WrongEnvironment)
    );
    let old = a.font_snapshot();
    metrics::install_probe(|_, size| (size, size, size, size), a.values());
    assert_eq!(
        backend.metrics(request, &old),
        Err(FontFactsError::StaleSnapshot)
    );
    let current = a.font_snapshot();
    assert!(backend.metrics(request, &current).is_ok());
    a.install_font_facts(None);
    assert_eq!(
        backend.metrics(request, &current),
        Err(FontFactsError::StaleSnapshot)
    );
    let weak = Rc::downgrade(&a);
    drop(a);
    assert!(weak.upgrade().is_none());
    assert_eq!(
        backend.normal_line(request, &current),
        Err(FontFactsError::Unavailable)
    );
}

#[test]
fn adapter_retains_exact_metrics_and_normal_rounding() {
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    metrics::install_probe(
        |_, size| (size * 0.63, size * 0.47, size * 1.17, size * 1.03),
        vcx,
    );
    metrics::install_vprobe(|_, size| (size * 0.89, size * 0.22, size * 0.71), vcx);
    for family in ["Face", "", "Other, serif"] {
        for size in [0.0, 16.0, 19.3, 40.0] {
            let request = FontRequest::new(family, size);
            let facts = doc
                .font_provider()
                .metrics(request, &doc.font_snapshot())
                .unwrap();
            let (ch, ex) = metrics::ch_ex_px(family, size, vcx);
            assert_eq!(facts.zero_advance.unwrap().px.to_bits(), ch.to_bits());
            assert_eq!(facts.x_height.unwrap().px.to_bits(), ex.to_bits());
            assert_eq!(
                facts.cap_height.unwrap().px.to_bits(),
                metrics::vmetrics_px(family, size, vcx).2.to_bits()
            );
            assert_eq!(
                facts.ideographic_advance.unwrap().px.to_bits(),
                metrics::ic_px(family, size, vcx).to_bits()
            );
            let fraction = metrics::normal_line(family, vcx);
            assert_eq!(
                root_normal_line_px(family, size, vcx).to_bits(),
                metrics::normal_px(fraction, size, vcx).to_bits()
            );
        }
    }
}
