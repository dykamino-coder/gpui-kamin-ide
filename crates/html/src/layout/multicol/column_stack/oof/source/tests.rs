//! Прямой OOF начинает независимый поток без normal fragment владельца.

use super::*;
use crate::dom::Node;
use crate::layout::options::RenderOpts;
use gpui::{point, px, size};

#[gpui::test]
fn direct_oof_uses_native_static_anchor_and_keeps_empty_normal_plan(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let tree = crate::dom::parse(
            "<section style='position:relative;width:100px'><div style='height:15px'></div><aside style='position:absolute;left:7px;width:20px;height:30px'></aside></section>",
            "",
        );
        let mut opts = RenderOpts {
            viewport: (400., 400.),
            normal_line_height: 1.2,
            text: Default::default(),
            cx: Default::default(),
            frag: Default::default(),
        };
        opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
        opts.cx.values.viewport = opts.viewport;
        crate::style::cascade::materialize::materialize(
            &tree, opts.root_style().style_rc(), opts.cx.vcx(),
        );
        let mut pending: Vec<_> = tree.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e), _ => None,
        }).collect();
        let host = loop {
            let e = pending.pop().unwrap();
            if e.tag == "section" { break e; }
            pending.extend(e.children.iter().filter_map(|n| match n {
                Node::Element(e) => Some(e), _ => None,
            }));
        };
        let kids: Vec<_> = host.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e.clone()), _ => None,
        }).collect();
        let geometry = crate::layout::positioned::containing_block::metrics::layout::collect(
            &kids, &crate::style::computed::BoxStyle::shared(host.style.used()), 100., &opts, window, app,
        );
        assert_eq!(geometry.flows.len(), 1);
        assert_eq!(geometry.flows[0].owner, 0);
        let expected = geometry.flows[0].metrics.normal_rect();
        let stack = ColumnStack::new(
            Vec::new(), 2, 0., Some(20.), None, None, None, Default::default(),
        ).with_oof(Some(geometry), Default::default());
        let normal = Plan::default();
        let (_, inputs) = stack.oof_inputs(&normal).unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].first, 0);
        assert_eq!(inputs[0].block_start, f32::from(expected.origin.y));
        assert_eq!(inputs[0].inline_start, 7.);
        assert!(normal.fragments.is_empty());
        gpui::div()
    });
}

#[gpui::test]
fn native_source_routes_only_a_stack_on_its_own_axis(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let tree = crate::dom::parse(
            "<section style='writing-mode:vertical-rl;direction:rtl;position:relative;height:100px'><aside style='position:absolute;width:20px;height:30px'></aside></section>",
            "",
        );
        let mut opts = RenderOpts {
            viewport: (400., 400.),
            normal_line_height: 1.2,
            text: Default::default(),
            cx: Default::default(),
            frag: Default::default(),
        };
        opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
        opts.cx.values.viewport = opts.viewport;
        crate::style::cascade::materialize::materialize(
            &tree, opts.root_style().style_rc(), opts.cx.vcx(),
        );
        let mut pending: Vec<_> = tree.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e), _ => None,
        }).collect();
        let host = loop {
            let e = pending.pop().unwrap();
            if e.tag == "section" { break e; }
            pending.extend(e.children.iter().filter_map(|n| match n {
                Node::Element(e) => Some(e), _ => None,
            }));
        };
        let kids: Vec<_> = host.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e.clone()), _ => None,
        }).collect();
        let geometry = crate::layout::positioned::containing_block::metrics::layout::collect(
            &kids, &crate::style::computed::BoxStyle::shared(host.style.used()), 100., &opts, window, app,
        );
        // The vertical `direction: rtl` columns run on the horizontal stack axis.
        assert!(geometry.native_along(true).is_some());
        assert!(geometry.native_along(false).is_none());
        gpui::div()
    });
}
