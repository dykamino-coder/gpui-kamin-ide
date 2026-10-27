//! Проверки статической точки по завершённой строке без зависимости от системных шрифтов.

use super::*;
use std::sync::Arc;

fn paragraph(text: &str, align: Align, rtl: bool) -> (Paragraph, Vec<Seg>) {
    let mut p = Paragraph::new(
        text.to_owned().into(),
        vec![],
        px(16.),
        px(20.),
        align,
        Wrap {
            rtl,
            ..Default::default()
        },
    );
    let segs = text
        .char_indices()
        .enumerate()
        .map(|(i, (start, ch))| Seg {
            start,
            end: start + ch.len_utf8(),
            offset: px(i as f32 * 10.),
            layout: Arc::new(gpui::LineLayout {
                width: px(10.),
                ..Default::default()
            }),
        })
        .collect();
    p.lines = vec![Line::test(
        0..text.len(),
        px(text.chars().count() as f32 * 10.),
    )];
    (p, segs)
}

#[test]
fn inline_static_point_observes_alignment_and_visual_bidi_order() {
    for (align, expected) in [
        (Align::Left, 10.),
        (Align::Center, 45.),
        (Align::Right, 80.),
    ] {
        let (p, segs) = paragraph("abc", align, false);
        let (rect, line) = rect(&p, &segs, &[(0., 0.)], 1, px(100.), Position::Inline);
        assert_eq!(rect.origin, point(px(expected), px(0.)));
        assert_eq!(rect.size, size(px(0.), px(20.)));
        assert_eq!(line.size, size(px(100.), px(20.)));
    }
    let (p, segs) = paragraph("אב", Align::Right, true);
    for (at, expected) in [(0, 100.), (2, 90.), (4, 80.)] {
        assert_eq!(
            rect(&p, &segs, &[(0., 0.)], at, px(100.), Position::Inline)
                .0
                .origin
                .x,
            px(expected)
        );
    }
    let (p, segs) = paragraph("aאבb", Align::Left, false);
    assert_eq!(
        rect(&p, &segs, &[(0., 0.)], 3, px(100.), Position::Inline)
            .0
            .origin
            .x,
        px(20.)
    );
}

#[test]
fn static_line_edges_exclude_clearance_and_block_uses_the_finished_line_step() {
    let (mut p, segs) = paragraph("ab", Align::Left, false);
    p.lines = vec![Line::test(0..1, px(10.)), Line::test(1..2, px(10.))];
    p.lines[1].clearance = px(40.);
    let pads = [(2., 6.), (44., 0.)];
    let (inline, line) = rect(&p, &segs, &pads, 1, px(100.), Position::Inline);
    assert_eq!(inline.origin.y, px(68.));
    assert_eq!(line.size.height, px(24.));
    assert_eq!(
        rect(&p, &segs, &pads, 2, px(100.), Position::Block).0,
        Bounds::new(point(px(0.), px(92.)), size(px(100.), px(0.)))
    );
}

#[test]
fn phantom_line_has_no_height_and_counts_float_opportunity_once() {
    let (mut p, _) = paragraph("", Align::Center, false);
    p.lines.clear();
    p.flow = Arc::new((
        vec![crate::layout::float::shapes::FloatShape::Band {
            top: 0.,
            h: 100.,
            w: 40.,
        }],
        vec![],
    ));
    let (inline, line) = rect(&p, &[], &[], 0, px(100.), Position::Inline);
    assert_eq!(
        inline,
        Bounds::new(point(px(70.), px(0.)), size(px(0.), px(0.)))
    );
    assert_eq!(line.size.height, px(0.));
    assert_eq!(
        rect(&p, &[], &[], 0, px(100.), Position::Block)
            .0
            .size
            .width,
        px(100.)
    );
    let (mut p, segs) = paragraph("a", Align::Left, false);
    p.flow = Arc::new((
        vec![crate::layout::float::shapes::FloatShape::Band {
            top: 0.,
            h: 100.,
            w: 40.,
        }],
        vec![],
    ));
    p.lines[0].indent = px(40.);
    assert_eq!(
        rect(&p, &segs, &[(0., 0.)], 0, px(100.), Position::Inline)
            .0
            .origin
            .x,
        px(40.)
    );
}
