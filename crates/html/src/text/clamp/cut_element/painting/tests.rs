//! Тесты планов clamp по неизменяемым снимкам опросов, независимо от окна и реестров.

use super::ClampEntry;
use super::geometry::ClampGeometry;
use gpui::{Bounds, point, px, size};
use std::collections::HashMap;

fn paragraph(seq: u32, y: f32, h: f32) -> ClampEntry {
    ClampEntry {
        bounds: Bounds::new(point(px(0.0), px(y)), size(px(100.0), px(h))),
        line: 10.0,
        seq: Some(seq),
        skip_count: false,
        fixed_height: false,
        bp_after: 0.0,
        end_margin: 0.0,
        min_end: 0.0,
        capped: false,
        node: 0,
        clamped: None,
        empty: false,
        span: None,
        bt_before: 0.0,
        positioned: false,
    }
}

#[test]
fn counted_plan_uses_actual_rows_and_is_repeatable() {
    let geometry = ClampGeometry {
        limit: Some(2),
        max_h: None,
        trim_end: 0.0,
    };
    let entries = vec![paragraph(0, 100.0, 45.0)];
    let rows = HashMap::from([(0, vec![(100.0, 111.0), (111.0, 123.0), (123.0, 145.0)])]);
    let plan = geometry.compute(entries.clone(), &rows, 100.0, None);
    assert_eq!(plan.para, Some((0, 2)));
    assert_eq!(plan.cut, f32::INFINITY);
    assert_eq!(geometry.compute(entries, &rows, 100.0, None), plan);
    assert_eq!(rows[&0].len(), 3);
}

#[test]
fn automatic_budget_preserves_a_previous_smaller_budget() {
    let geometry = ClampGeometry {
        limit: None,
        max_h: Some(30.0),
        trim_end: 0.0,
    };
    let plan = geometry.compute(
        vec![paragraph(0, 0.0, 50.0)],
        &HashMap::new(),
        0.0,
        Some((0, 2)),
    );
    assert_eq!(plan.para, Some((0, 2)));
    assert_eq!(plan.cut, 30.0);
}

#[test]
fn balanced_taller_rows_reclamp_without_previous_frame_budget() {
    let geometry = ClampGeometry {
        limit: None,
        max_h: Some(30.0),
        trim_end: 0.0,
    };
    let rows = HashMap::from([(0, vec![(0.0, 20.0), (20.0, 30.0), (30.0, 50.0)])]);
    let initial = geometry.compute(vec![paragraph(0, 0.0, 50.0)], &rows, 0.0, None);
    assert_eq!(initial.para, Some((0, 2)));
    let mut applied = paragraph(0, 0.0, 40.0);
    applied.clamped = Some(2);
    let balanced_rows = HashMap::from([(0, vec![(0.0, 20.0), (20.0, 40.0)])]);
    let refined = geometry.compute(vec![applied], &balanced_rows, 0.0, None);
    assert_eq!(refined.para, Some((0, 1)));
    assert_eq!(refined.cut, 20.0);
}

#[test]
fn empty_box_after_last_line_places_point_between_paragraphs() {
    let geometry = ClampGeometry {
        limit: None,
        max_h: Some(25.0),
        trim_end: 0.0,
    };
    let mut empty = paragraph(0, 22.0, 0.0);
    empty.seq = None;
    empty.empty = true;
    let plan = geometry.compute(
        vec![paragraph(0, 0.0, 20.0), empty, paragraph(1, 30.0, 20.0)],
        &HashMap::new(),
        0.0,
        None,
    );
    assert_eq!(plan.para, None);
    assert_eq!(plan.next, Some(1));
    assert_eq!(plan.extent, Some(20.0));
    assert_eq!(plan.cut, 25.0);
}

#[test]
fn fragmented_block_reports_its_used_size_without_mutating_the_probe() {
    let geometry = ClampGeometry {
        limit: Some(2),
        max_h: None,
        trim_end: 0.0,
    };
    let mut block = paragraph(0, 0.0, 50.0);
    block.line = 0.0;
    block.seq = None;
    block.node = 7;
    block.span = Some((0, 1));
    block.bp_after = 4.0;
    let probes = vec![block, paragraph(0, 0.0, 50.0), paragraph(1, 50.0, 10.0)];
    let plan = geometry.compute(probes.clone(), &HashMap::new(), 0.0, None);
    assert_eq!(plan.sizes, vec![(7, 24.0)]);
    assert_eq!(plan.nodes, vec![7]);
    assert_eq!(plan.para, Some((0, 2)));
    assert_eq!(plan.extent, Some(20.0));
    assert_eq!(plan.cut, 24.0);
    assert_eq!(f32::from(probes[0].bounds.size.height), 50.0);
}

#[test]
fn small_cut_change_is_recorded_without_requesting_a_frame() {
    use super::{CutUpdate, cut_update};
    let small = cut_update(Some(100.0), 103.0);
    assert_eq!(
        small,
        CutUpdate {
            write: true,
            reframe: false
        }
    );
    let large = cut_update(Some(100.0), 105.0);
    assert!(large.write && large.reframe);
    let same = cut_update(Some(100.0), 100.0);
    assert!(!same.write && !same.reframe);
    let first = cut_update(None, f32::INFINITY);
    assert!(first.write && first.reframe);
    let unclamped = cut_update(Some(100.0), f32::INFINITY);
    assert!(unclamped.write && unclamped.reframe);
    let stable_inf = cut_update(Some(f32::INFINITY), f32::INFINITY);
    assert!(!stable_inf.write && !stable_inf.reframe);
}
