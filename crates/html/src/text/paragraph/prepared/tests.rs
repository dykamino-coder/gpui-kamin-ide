//! Регрессии границ прогонов и инвалидирования модели ревизии.
use super::*;
use crate::text::paragraph::{Align, Wrap};
use gpui::{TextRun, px};

#[test]
fn edge_containment_extrema_match_every_original_span() {
    let mut p = Paragraph::new(
        "abcdefghij".into(),
        [0, 2, 3, 0, 5]
            .into_iter()
            .map(|len| TextRun {
                len,
                ..Default::default()
            })
            .collect(),
        px(16.),
        px(20.),
        Align::Left,
        Wrap::default(),
    );
    p.edge_spans = vec![
        (0..10, true, 20.),
        (2..5, false, 30.),
        (1..9, true, 40.),
        (0..2, false, 10.),
        (2..2, true, 50.),
    ];
    let prepared = p.prepared();
    for (i, run) in p.runs.iter().enumerate() {
        let end = prepared.run_ends[i];
        let start = end - run.len;
        let expected = p
            .edge_spans
            .iter()
            .filter(|(r, _, _)| r.start <= start && r.end >= end)
            .map(|(r, _, _)| (r.start, r.end))
            .reduce(|a, b| (a.0.max(b.0), a.1.min(b.1)));
        assert_eq!(prepared.run_edges[i], expected);
    }
    let key = p.measure_key();
    let p = p.word_spacing(px(2.));
    assert_ne!(p.measure_key(), key);
    assert!(p.prepared().words.first(0).is_none());
}
