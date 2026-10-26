//! Equivalence of metric readers, query scopes and cache revisions.

use super::*;
use crate::text::metrics;
use std::cell::RefCell;

fn document(log: Rc<RefCell<Vec<char>>>) -> Rc<DocState> {
    let doc = DocState::new(Default::default());
    let h = log.clone();
    metrics::install_probe(
        move |_, size| {
            h.borrow_mut().push('h');
            (size * 0.3, size * 0.45, size * 1.23, size * 1.1)
        },
        doc.values(),
    );
    metrics::install_vprobe(
        move |_, size| {
            log.borrow_mut().push('v');
            (size * 0.81, size * 0.19, size * 0.68)
        },
        doc.values(),
    );
    doc
}

#[test]
fn scoped_queries_preserve_cold_warm_order_and_generation() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let doc = document(log.clone());
    let provider = doc.font_provider();
    let snapshot = doc.font_snapshot();
    let mut r = FontRequest::new("Ahem", 19.3);
    r.metric_query = FontMetricQuery::Vertical;
    let vertical = provider.metrics(r, &snapshot).unwrap();
    assert!(vertical.zero_advance.is_none());
    assert_eq!(*log.borrow(), vec!['v']);
    r.metric_query = FontMetricQuery::Horizontal;
    let horizontal = provider.metrics(r, &snapshot).unwrap();
    assert!(horizontal.ascent.is_none());
    assert_eq!(*log.borrow(), vec!['v', 'h']);
    provider.metrics(r, &snapshot).unwrap();
    assert_eq!(*log.borrow(), vec!['v', 'h']);
    r.metric_query = FontMetricQuery::Ideographic;
    let ic = provider.metrics(r, &snapshot).unwrap();
    assert!(ic.x_height.is_none());
    assert_eq!(*log.borrow(), vec!['v', 'h']);
    assert_eq!(
        horizontal.zero_advance.unwrap().px.to_bits(),
        metrics::ch_ex_px("Ahem", 19.3, doc.values()).0.to_bits()
    );
    assert_eq!(
        ic.ideographic_advance.unwrap().px.to_bits(),
        metrics::ic_px("Ahem", 19.3, doc.values()).to_bits()
    );
    metrics::install_probe(|_, size| (size, size, size, size), doc.values());
    assert_eq!(
        provider.metrics(r, &snapshot),
        Err(FontFactsError::StaleSnapshot)
    );
    let current = doc.font_snapshot();
    assert_ne!(snapshot.generation(), current.generation());
    let replacement = provider.metrics(r, &current).unwrap();
    assert_eq!(replacement.ideographic_advance.unwrap().px, 19.3);
}

#[test]
fn unit_and_normal_readers_remain_bit_exact_in_both_cache_states() {
    for family in ["Ahem", "", "Missing"] {
        for size in [0.0, 16.0, 19.3, 40.0] {
            let direct = document(Rc::default());
            let adapted = document(Rc::default());
            for _ in 0..2 {
                let old = direct.values();
                let new = adapted.values();
                let (ch, ex) = metrics::ch_ex_px(family, size, old);
                assert_eq!(
                    crate::style::font_metrics::horizontal(family, size, new),
                    (ch, ex)
                );
                let ic = metrics::ic_px(family, size, old);
                assert_eq!(
                    crate::style::font_metrics::ideographic(family, size, new).to_bits(),
                    ic.to_bits()
                );
                let v = metrics::vmetrics_px(family, size, old);
                assert_eq!(crate::style::font_metrics::vertical(family, size, new), v);
                let fraction = metrics::normal_line(family, old);
                let facts = crate::style::font_metrics::normal(family, size, new);
                assert_eq!(facts.fraction.unwrap().to_bits(), fraction.to_bits());
                assert_eq!(
                    facts.height.unwrap().px.to_bits(),
                    metrics::normal_px(fraction, size, old).to_bits()
                );
                assert_eq!(
                    facts.content_height.map(|m| m.px),
                    metrics::content_px(fraction, size, old)
                );
            }
        }
    }
}

#[test]
fn metric_descriptors_are_immutable_and_document_scoped() {
    use crate::text::fonts::metric_overrides;
    let a = DocState::new(Default::default());
    let b = DocState::new(Default::default());
    metric_overrides::register(
        "same",
        0,
        "ascent-override:50%;line-gap-override:0%",
        a.values(),
    );
    metric_overrides::register("same", 0, "ascent-override:150%", b.values());
    let old = a.font_snapshot();
    let other = b.font_snapshot();
    assert_eq!(
        old.metric_overrides("same", 0),
        [Some(0.5), None, Some(0.0)]
    );
    assert_eq!(other.metric_overrides("same", 0), [Some(1.5), None, None]);
    metric_overrides::clear(a.values());
    metric_overrides::register("same", 0, "descent-override:25%", a.values());
    let current = a.font_snapshot();
    assert_ne!(old.generation(), current.generation());
    assert_eq!(
        old.metric_features("same", 0),
        vec![
            ("kasc".into(), 0.5f32.to_bits()),
            ("kgap".into(), 0.0f32.to_bits())
        ]
    );
    assert_eq!(
        current.metric_overrides("same", 0),
        [None, Some(0.25), None]
    );
    assert_eq!(other.metric_overrides("same", 0), [Some(1.5), None, None]);
}

#[test]
fn zero_normal_overrides_keep_css_and_form_policies_distinct() {
    use crate::style::values::value::{Len, root_normal_line_px};
    let doc = document(Rc::default());
    crate::text::fonts::metric_overrides::register(
        "zero",
        0,
        "ascent-override:0%;descent-override:0%;line-gap-override:0%",
        doc.values(),
    );
    let vcx = doc.values();
    let f = crate::style::font_metrics::normal("zero", 20.0, vcx);
    assert_eq!(f.fraction, Some(0.0));
    assert_eq!(f.height.unwrap().px, 0.0);
    assert_eq!(root_normal_line_px("zero", 20.0, vcx), 24.0);
    let mut c = crate::style::computed::Computed::default();
    c.font_family = Some("zero".into());
    c.font_size = Some(Len::Px(20.0));
    let c = crate::style::computed::BoxStyle::from(c);
    assert_eq!(
        crate::style::ua::forms::text_style(&c, vcx).line_height,
        Some(Len::Px(0.0))
    );
}
