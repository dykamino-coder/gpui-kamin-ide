//! Сравнение всех точек разрыва и координат IFC с настоящей раскладкой.

use crate::layout::fragment::{flow::blocks::Laid, types::Kid};

pub(super) fn reject_mixed(
    model: &super::Model,
    mut grows: Vec<crate::layout::fragment::push::Growth>,
) {
    let state = model.state();
    grows.push(crate::layout::fragment::push::Growth {
        target: crate::layout::fragment::flow::blocks::Pushed::Box(u64::MAX),
        at: 0.,
        amount: 1.,
        forced: false,
    });
    assert!(!model.accepts(&grows));
    assert_eq!(model.state(), state);
    assert!(model.uncommitted());
}

fn number(a: f32, b: f32) {
    assert!((a - b).abs() < 0.01, "{a} != {b}");
}

fn ranges(a: &[(f32, f32)], b: &[(f32, f32)]) {
    assert_eq!(a.len(), b.len(), "{a:?} != {b:?}");
    for (a, b) in a.iter().zip(b) {
        number(a.0, b.0);
        number(a.1, b.1);
    }
}

pub(super) fn kid(a: &Kid, b: &Kid) {
    number(a.h, b.h);
    number(a.over, b.over);
    assert_eq!(a.forced.len(), b.forced.len());
    for (&a, &b) in a.forced.iter().zip(&b.forced) {
        number(a, b);
    }
    for (a, b) in [
        (&a.cuts, &b.cuts),
        (&a.solid, &b.solid),
        (&a.keep, &b.keep),
        (&a.avoid, &b.avoid),
        (&a.line_keep, &b.line_keep),
    ] {
        ranges(a, b);
    }
    assert!(a.table_rows.is_empty() && b.table_rows.is_empty());
}

pub(super) fn laid(a: &Laid, b: &Laid) {
    assert_eq!(a.boxes.len(), b.boxes.len());
    for (id, &bounds) in &a.boxes {
        ranges(&[bounds], &[b.boxes[id]]);
    }
    for (id, frame) in &a.flexes {
        let actual = &b.flexes[id];
        assert_eq!(frame.column, actual.column);
        assert_eq!(frame.lines.len(), actual.lines.len());
        for (line, actual) in frame.lines.iter().zip(&actual.lines) {
            assert_eq!(line.items, actual.items);
            assert_eq!(line.order, actual.order);
            ranges(&[(line.start, line.end)], &[(actual.start, actual.end)]);
        }
    }
    for (id, lines) in &a.content {
        let actual = &b.content[id];
        assert_eq!(lines.len(), actual.len());
        for (line, actual) in lines.iter().zip(actual) {
            assert_eq!(line.paragraph, actual.paragraph);
            assert_eq!(line.byte, actual.byte);
            assert_eq!(line.state, actual.state);
            number(line.top, actual.top);
            number(line.bottom, actual.bottom);
            number(line.prefix, actual.prefix);
            number(line.consumed_before, actual.consumed_before);
        }
    }
}
