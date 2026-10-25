//! Содержимое строки, начальное состояние и продолжение абзаца.
use super::*;
use crate::text::paragraph::{Align, Line, Wrap};
use gpui::px;

#[test]
fn break_before_first_line_does_not_consume_collapsed_leading_spaces() {
    let flow = Rc::new(FlowMeasure::for_lines());
    let mut paragraph = Paragraph::new(
        "  a b".into(),
        Vec::new(),
        px(16.),
        px(16.),
        Align::Left,
        Wrap::default(),
    )
    .fragment_flow(Some(flow), StackAxis::Horizontal, (2, 2));
    paragraph.indent = crate::text::paragraph::Indent {
        px: 17.,
        ..Default::default()
    };
    paragraph.fragment.as_ref().unwrap().node.resume.resume(
        crate::layout::fragment::token::fc::InlineState {
            text_pos: 2,
            ..Default::default()
        },
    );
    let lines = paragraph.lay(None, &[]);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].range, 2..5);
    assert_eq!(lines[0].indent, px(17.));
}

#[test]
fn continued_each_line_indent_requires_a_hard_text_break() {
    let flow = Rc::new(FlowMeasure::for_lines());
    let mut paragraph = Paragraph::new(
        "a b".into(),
        Vec::new(),
        px(16.),
        px(16.),
        Align::Left,
        Wrap::default(),
    )
    .fragment_flow(Some(flow), StackAxis::Horizontal, (2, 2));
    paragraph.indent = crate::text::paragraph::Indent {
        px: 17.,
        each_line: true,
        ..Default::default()
    };
    let cursor = &paragraph.fragment.as_ref().unwrap().node.resume;
    for hard in [false, true] {
        cursor.resume(crate::layout::fragment::token::fc::InlineState {
            text_pos: 2,
            line_idx: 1,
            past_first_line: true,
            after_hard_break: hard,
            ..Default::default()
        });
        let lines = paragraph.lay_in(2..3, None, &[]);
        assert_eq!(lines[0].indent, px(if hard { 17. } else { 0. }));
    }
}

#[test]
fn float_avoidance_space_can_split_between_line_grid_positions() {
    let mut paragraph = Paragraph::new(
        "ab".into(),
        Vec::new(),
        px(16.0),
        px(16.0),
        Align::Left,
        Wrap::default(),
    );
    paragraph.lines = [0..1, 1..2]
        .into_iter()
        .enumerate()
        .map(|(i, range)| Line {
            clearance: px(if i == 1 { 32. } else { 0. }),
            range,
            width: px(0.),
            ellipsis: false,
            clamped: false,
            vis_cut: None,
            hyphen: false,
            indent: px(0.),
        })
        .collect();
    let node = FlowNode::new(Rc::new(FlowMeasure::for_lines()), StackAxis::Horizontal);
    paragraph.fragment = Some(LineFlow {
        node: node.clone(),
        keep: (1, 1),
    });
    paragraph.publish_fragment_lines(&[(0., 0.), (32., 0.)], 16.);
    let geometry = node.geometry.borrow();
    assert_eq!(geometry.solid, [(0., 16.), (48., 64.)]);
    assert!(geometry.cuts.contains(&(48., 48.)));
    assert!(geometry.cuts.contains(&(16., 16.)));
    assert!(geometry.cuts.contains(&(64., 64.)));
}
#[test]
fn consumed_space_moves_only_the_continuation_and_survives_rebuild() {
    let flow = Rc::new(FlowMeasure::for_lines());
    let mut paragraph = Paragraph::new(
        "ab".into(),
        Vec::new(),
        px(20.),
        px(20.),
        Align::Left,
        Wrap::default(),
    );
    paragraph.lines = [0..1, 1..2]
        .into_iter()
        .map(|range| Line {
            clearance: px(0.),
            range,
            width: px(0.),
            ellipsis: false,
            clamped: false,
            vis_cut: None,
            hyphen: false,
            indent: px(0.),
        })
        .collect();
    paragraph = paragraph.fragment_flow(Some(flow.clone()), StackAxis::Horizontal, (1, 1));
    let node = paragraph.fragment.as_ref().unwrap().node.clone();
    let push = crate::layout::fragment::flow::LinePush {
        at: 20.,
        line: 1,
        growth: node.growth.clone(),
    };
    assert!(push.grow(15.));
    let pads = paragraph.line_padding();
    paragraph.publish_fragment_lines(&pads, 20.);
    assert_eq!(pads, [(0., 0.), (15., 0.)]);
    assert_eq!(node.geometry.borrow().solid, [(0., 20.), (35., 55.)]);
    assert_eq!(node.geometry.borrow().cuts, [(35., 35.), (55., 55.)]);
    let inline = &node.geometry.borrow().inline[0];
    let mut space = crate::layout::fragment::token::Space::pages();
    space.block_size = Some(35.);
    let mut store = crate::layout::fragment::token::TokenStore::default();
    let candidate = inline.resume(space, 0., None, &mut store, false).unwrap();
    assert_eq!(candidate.end, 35.);
    let next = candidate.next.unwrap();
    assert_eq!(store.get(next).unwrap().consumed, 35.);
    space.block_size = Some(20.);
    let continuation = inline
        .resume(space, 0., Some(next), &mut store, false)
        .unwrap();
    assert_eq!(continuation.end, 55.);
    assert!(continuation.next.is_none());
    flow.rewind();
    let rebuilt = FlowNode::new(flow.clone(), StackAxis::Horizontal);
    assert!(Rc::ptr_eq(&rebuilt.growth, &node.growth));
    let spare = FlowNode::new(Rc::new(flow.constraints_only()), StackAxis::Horizontal);
    assert!(Rc::ptr_eq(&spare.growth, &node.growth));
    let content = node.content.borrow();
    assert_eq!(
        content
            .iter()
            .map(|l| (l.paragraph, l.byte, l.top))
            .collect::<Vec<_>>(),
        [(0, 0, 0.), (0, 1, 35.)]
    );
    flow.resume_lines(&content, 20.);
    assert_eq!(rebuilt.resume.get(), 1);
    assert_eq!(spare.resume.get(), 0);
}
