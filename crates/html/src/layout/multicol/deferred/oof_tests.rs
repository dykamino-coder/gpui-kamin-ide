//! Исходный OOF layout проходит через measured children в отдельный план колонок.

use super::*;
use crate::layout::fragment::plan::{Completion, Plan};
use crate::layout::fragment::types::Frag;
use gpui::{Bounds, point, px, size};

#[gpui::test]
fn absolute_continuation_and_balance_preserve_the_normal_neighbour(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let tree = crate::dom::parse("<section style='column-count:2;column-fill:auto;width:210px;height:70px'><div style='position:relative;height:20px'><div style='position:absolute;top:65px;width:80px;height:85px'></div></div><p style='height:20px;margin:0'></p></section>", "");
        let mut opts = RenderOpts { viewport: (400., 400.), normal_line_height: 1.2, text: Default::default(), cx: Default::default(), frag: Default::default() };
        opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
        opts.cx.values.viewport = opts.viewport;
        crate::style::cascade::materialize::materialize(&tree, opts.root_style().style_rc(), opts.cx.vcx());
        let host = super::tests::section(&tree);
        let merged = crate::style::cascade::materialize::box_style(&opts.root_style(), &host.style, opts.cx.vcx());
        // OOF пока исключает deferred recipe: используем действующий production
        // путь stack_plan -> column_kids -> prepare -> stack_children.
        let measured = crate::layout::multicol::measure::stack_plan(
            host, &merged, &opts.root_style(), &opts, 2, 10., false, false,
            Some(70.), Some(70.), None, None, None,
        );
        let kids = measured.stackable.unwrap();
        let source: Vec<_> = kids.iter().map(|(e, _)| e.clone()).collect();
        let geometry = crate::layout::positioned::containing_block::metrics::layout::collect(
            &source, &merged, 100., &opts, window, app,
        );
        opts.cx.position.geometry = Some(geometry.clone());
        let (parents, _, parallel, kids) = crate::layout::multicol::container::kids::column_kids(
            &merged, kids, 2, 10., false, None, 2, Some(70.),
            (Some(100.), Some(100.)), &opts,
        );
        let prepared = kids.iter().enumerate().map(|(i, (e, shape))| {
            crate::layout::multicol::stack_child::prepare(
                i, e, shape, parents[i].as_ref().unwrap_or(&merged), &opts,
                &parallel, false, false, Some(100.), Some(70.), &measured.measured_kids,
            )
        }).collect();
        let children = crate::layout::multicol::container::kids::stack_children(
            host, &merged, &opts, 2, false, false, (Some(100.), Some(100.)), None,
            measured.nested_auto, measured.nested_whole, measured.measured_kids,
            2, Some(70.), parents, parallel, kids, Vec::new(), prepared,
        );
        assert_eq!(children.len(), 2);
        assert_eq!(geometry.flows.len(), 1);
        let mut stack = ColumnStack::new(children, 2, 10., Some(70.), None, None, None, None, Default::default()).with_oof(Some(geometry), merged.wm());
        stack.col_w.set(100.);
        for i in 0..stack.children.len() { stack.lay_out_main(i, window, app); }
        *stack.lines_plan.borrow_mut() = vec![(0., 70.)];
        let normal = Plan { fragments: vec![
            Frag { kid: 0, copy: 0, col: 0, y: 0., from: 0., h: 20., head: 0., foot: 0. },
            Frag { kid: 1, copy: 0, col: 0, y: 20., from: 0., h: 20., head: 0., foot: 0. },
        ], ..Default::default() };
        stack.plan_oof(&normal, Bounds { origin: point(px(0.), px(0.)), size: size(px(210.), px(70.)) });
        let plan = stack.oof.plan.borrow();
        assert_eq!(plan.completion, Completion::Complete);
        assert_eq!(plan.failure, None);
        assert_eq!(plan.iter().map(|f| (f.col, f.y, f.from, f.h)).collect::<Vec<_>>(), [(0, 65., 0., 5.), (1, 0., 5., 70.), (2, 0., 75., 10.)]);
        assert_eq!(normal[1].y, 20.);
        assert_eq!(normal.len(), 2);
        assert_eq!(stack.children[0].h, 20.);
        drop(plan);
        stack.fixed_height = None;
        // Normal-владелец не зависит от наличия block recorder.
        stack.children[0].watched = None;
        let kids = stack.kid_geoms();
        for _ in 0..2 {
            let (height, _, normal, _) = stack.balance(&kids);
            assert_eq!(height, 75.);
            assert_eq!(normal.completion, Completion::Complete);
            assert_eq!(normal.failure, None);
            assert_eq!(stack.children[0].h, 20.);
            assert_eq!(stack.children[1].h, 20.);
        }
        let source = &mut std::rc::Rc::make_mut(&mut stack.oof.geometry).flows[0];
        source.kid.h = 80.;
        source.kid.over = 80.;
        source.kid.monolith = true;
        source.metrics.normal_offset.y = px(20.);
        for _ in 0..2 {
            let (height, _, plan, _) = stack.balance(&kids);
            assert_eq!(height, 80.);
            assert_eq!(plan.completion, Completion::Complete);
            assert_eq!(plan.failure, None);
        }
        gpui::div()
    });
}
