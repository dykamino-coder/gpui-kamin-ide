//! Тесты: фантомные абзацы хранят строчную статическую точку, не создавая строки.

use crate::layout::float::shapes::FloatShape;
use crate::text::paragraph::*;
use gpui::{Bounds, point, px, size};
use std::sync::Arc;

#[test]
fn a_phantom_paragraph_counts_no_line_and_uses_each_float_opportunity_once() {
    for rtl in [false, true] {
        for right_float in [false, true] {
            for (align, advance) in [
                (Align::Left, 0.0),
                (Align::Center, 25.0),
                (Align::Right, 50.0),
            ] {
                let shape = FloatShape::Band {
                    top: 0.0,
                    h: 50.0,
                    w: 50.0,
                };
                let cuts = if right_float {
                    (vec![], vec![shape])
                } else {
                    (vec![shape], vec![])
                };
                let mut paragraph = Paragraph::new(
                    "".into(),
                    vec![],
                    px(16.0),
                    px(18.0),
                    align,
                    Wrap {
                        rtl,
                        ..Default::default()
                    },
                )
                .flow_shapes(Arc::new(cuts));
                paragraph.lines = paragraph.lay(Some(px(100.0)), &[]);
                assert!(paragraph.lines.is_empty());
                let bounds = Bounds::new(point(px(8.0), px(100.0)), size(px(100.0), px(0.0)));
                let expected = 8.0 + if right_float { 0.0 } else { 50.0 } + advance;
                assert_eq!(
                    paragraph.point_of(&[], 0, bounds),
                    point(px(expected), px(100.0))
                );
                // A hypothetical block starts at the BFC edge, even beside
                // a float and under text alignment (CSS Position 3).
                assert_eq!(paragraph.next_line_point(0, bounds), bounds.origin);
            }
        }
    }
}
