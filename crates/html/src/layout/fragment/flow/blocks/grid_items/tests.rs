//! Продолжение элементов сетки со своего расхода по фрагментам.

use super::{ItemFrame, ResumedItem, resume_items};

#[test]
fn a_sibling_of_an_overflowing_monolith_resumes_from_its_own_consumed_size() {
    // Blink `unavailable_block_size`: three 150px monoliths overflow 100px columns and
    // stretch each grid fragment to 150; the 400px sibling shows 100px per column.
    let item = |id, end, barriers: Vec<(f32, f32)>| ResumedItem {
        grid: 9,
        last_track: 0,
        id,
        top: 0.0,
        end,
        barriers,
    };
    let monoliths = item(1, 450.0, vec![(0.0, 150.0), (150.0, 300.0), (300.0, 450.0)]);
    let sibling = item(2, 400.0, Vec::new());
    let frames: Vec<_> = (0..4)
        .map(|copy| ItemFrame {
            copy,
            start: 150.0 * copy as f32,
            end: 150.0 * (copy + 1) as f32,
            edge: 150.0 * copy as f32 + 100.0,
        })
        .collect();
    let r = resume_items(&[monoliths, sibling], &frames);
    assert!(!r.windows.keys().any(|k| k.1 == 1), "{:?}", r.windows);
    for copy in 0..3 {
        let w = r.windows[&(copy, 2)];
        assert_eq!(w.shift, 50.0 * copy as f32);
        assert_eq!(w.clip, Some(150.0 * copy as f32 + 100.0));
    }
    // The sibling ends 150px later than its content: the grid grows to hold it.
    assert_eq!(r.windows[&(3, 2)].shift, 150.0);
    assert_eq!(r.ends[&9], (0, 550.0));
}

#[test]
fn items_without_monolithic_overflow_keep_their_layout() {
    let item = ResumedItem {
        grid: 9,
        last_track: 1,
        id: 2,
        top: 0.0,
        end: 250.0,
        barriers: Vec::new(),
    };
    let frames: Vec<_> = (0..3)
        .map(|copy| ItemFrame {
            copy,
            start: 100.0 * copy as f32,
            end: 100.0 * (copy + 1) as f32,
            edge: 100.0 * (copy + 1) as f32,
        })
        .collect();
    let r = resume_items(&[item], &frames);
    assert!(r.windows.is_empty());
    assert_eq!(r.ends[&9], (1, 250.0));
}
