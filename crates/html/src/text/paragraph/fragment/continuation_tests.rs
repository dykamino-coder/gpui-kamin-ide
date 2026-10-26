//! Продолжение IFC при смене ширины: потреблённый текст не переносится повторно.
use super::*;
use crate::layout::float::shapes::FloatShape;
use crate::text::paragraph::{Align, Seg, Wrap};
use gpui::px;
use std::sync::Arc;

#[test]
fn width_change_projects_consumed_content_without_replaying_lines_against_floats() {
    for axis in [
        StackAxis::Horizontal,
        StackAxis::VerticalLr,
        StackAxis::VerticalRl,
    ] {
        let flow = Rc::new(FlowMeasure::for_lines());
        let mut p = Paragraph::new(
            "a a a a a a a a".into(),
            vec![],
            px(10.),
            px(20.),
            Align::Left,
            Wrap::default(),
        )
        .fragment_flow(Some(flow.clone()), axis, (1, 1));
        // Одинаковые продвижения исключают зависимость от системного шрифта.
        let segs: Vec<_> = (0..p.text.len())
            .map(|i| Seg {
                start: i,
                end: i + 1,
                offset: px(i as f32 * 10.),
                layout: Arc::new(gpui::LineLayout {
                    width: px(10.),
                    ..Default::default()
                }),
            })
            .collect();
        p.lines = p.lay(Some(px(80.)), &segs);
        assert_eq!(p.lines.len(), 2);
        p.publish_fragment_lines(&p.line_padding(), 20.);
        let node = p.fragment.as_ref().unwrap().node.clone();
        flow.resume_lines(&node.content.borrow(), 20.);
        assert_eq!(node.resume.get(), 8);
        assert_eq!(node.resume.consumed(), 20.);
        // Полосы уже спроецированы в координаты продолжения. Префикс
        // не должен ни потреблять их, ни перемещать начало оставшегося текста.
        p.flow = Arc::new((
            vec![FloatShape::Band {
                top: 20.,
                h: 20.,
                w: 20.,
            }],
            vec![],
        ));
        p.lines = p.lay(Some(px(60.)), &segs);
        assert_eq!(
            p.lines.iter().map(|l| l.range.clone()).collect::<Vec<_>>(),
            [8..11, 12..15]
        );
        assert_eq!(p.lines[0].indent, px(20.));
        assert_eq!(p.lines[1].indent, px(0.));
        p.publish_fragment_lines(&p.line_padding(), 20.);
        assert_eq!(node.geometry.borrow().solid, [(20., 40.), (40., 60.)]);
        assert_eq!(*node.starts.borrow(), [(1, 20.), (2, 40.)]);
        assert_eq!(p.flow_cut(0), (20., 0.));
        // Следующее продолжение сохраняет адрес и проекцию без накопления
        // прежних line boxes; рост строки адресуется исходным ordinal.
        flow.resume_lines(&node.content.borrow(), 40.);
        assert_eq!(node.resume.get(), 12);
        assert_eq!(node.resume.consumed(), 40.);
        p.flow = Arc::default();
        p.lines = p.lay(Some(px(100.)), &segs);
        assert_eq!(p.lines.len(), 1);
        assert_eq!(p.lines[0].range, 12..15);
        *node.growth.borrow_mut() = vec![0., 0., 5.];
        p.publish_fragment_lines(&p.line_padding(), 20.);
        assert_eq!(node.geometry.borrow().solid, [(45., 65.)]);
        assert_eq!(*node.starts.borrow(), [(2, 45.)]);
    }
}
