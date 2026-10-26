//! Продолжение после общего разрыва строк и блоков: пустая распорка не
//! переносится в новую колонку, содержимое параллельного потока сохраняется.

use super::{at, tree};
use crate::layout::fragment::flow::FlowGeometry;
use crate::layout::fragment::plan::{Space, fill_at};
use crate::layout::fragment::types::Kid;

#[test]
fn inline_end_preserves_block_resume_after_fragment_growth() {
    for contain in ["", "contain:style"] {
        for line in [8.0, 16.0, 20.0] {
            let (b, ids) = tree(&format!(
                "<div style='{contain}'><div style='break-after:column'>A<br>A</div><div>A<br>A</div></div>"
            ));
            let h = 2.0 * line;
            let boxes = at(&[(ids[0], 0.0, h), (ids[1], 2.0 * h, 3.0 * h)]);
            let mut lines = FlowGeometry::lines([line; 2]);
            lines.keep_lines(2, 2);
            lines.append(&lines.clone(), 2.0 * h);
            let m = b.breaks(3.0 * h, &boxes).finish(lines, 3.0 * h);
            assert_eq!(m.cuts, vec![(h, 2.0 * h)]);
            assert_eq!(m.forced, vec![h]);
            let k = Kid {
                h: 3.0 * h,
                over: 3.0 * h,
                cuts: m.cuts,
                forced: m.forced,
                solid: m.solid,
                ..Default::default()
            };
            let (_, _, plan) = fill_at(&[k], &|_| 2.0 * h, Space::columns(2));
            assert_eq!(plan.len(), 2);
            assert_eq!((plan[0].from, plan[0].h), (0.0, h));
            assert_eq!((plan[1].from, plan[1].h), (2.0 * h, h));
        }
    }
}

#[test]
fn parallel_content_prevents_skipping_a_shared_break_gap() {
    let mut lines = FlowGeometry {
        cuts: vec![(32.0, 32.0), (32.0, 64.0)],
        solid: vec![(0.0, 32.0), (32.0, 48.0), (64.0, 96.0)],
        ..Default::default()
    };
    lines.normalize();
    assert_eq!(lines.cuts, vec![(32.0, 32.0), (32.0, 64.0)]);
    let k = Kid {
        h: 96.0,
        over: 96.0,
        cuts: lines.cuts,
        solid: lines.solid,
        forced: vec![32.0],
        ..Default::default()
    };
    let (_, _, plan) = fill_at(&[k], &|_| 64.0, Space::columns(2));
    assert_eq!((plan[1].from, plan[1].h), (32.0, 64.0));
}
