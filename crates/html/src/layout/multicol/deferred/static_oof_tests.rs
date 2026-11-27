//! Direct positioned children retain their place across column breaks and rebuilds.

use super::*;
use crate::layout::fragment::plan::Completion;
use crate::layout::fragment::types::StackAxis;
use crate::style::values::value::Len;
use gpui::{Bounds, point, px, size};

#[gpui::test]
fn direct_static_anchors_survive_filtering_and_a_changed_normal_sibling(
    cx: &mut gpui::TestAppContext,
) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let tree = crate::document::parsed::parse(
            "<section style='position:relative;column-count:2;column-fill:auto;width:210px;height:70px'><aside style='position:absolute;width:10px;height:85px'></aside><div style='height:30px'></div><aside style='position:absolute;width:10px;height:85px'></aside><div style='height:20px;break-before:column'></div><aside style='position:absolute;width:10px;height:85px'></aside><aside style='position:absolute;top:7px;width:10px;height:85px'></aside></section>",
            "",
        );
        let mut opts = RenderOpts {
            viewport: (400., 400.), normal_line_height: 1.2,
            text: Default::default(), cx: Default::default(), frag: Default::default(),
        };
        opts.cx.values.doc = crate::layout::cx::BuildDoc::of(tree.doc.clone());
        opts.cx.values.viewport = opts.viewport;
        crate::style::cascade::materialize::materialize(
            &tree, opts.root_style().style_rc(), opts.cx.vcx(),
        );
        let mut host = super::tests::section(&tree).clone();
        let merged = crate::style::cascade::materialize::box_style(
            &opts.root_style(), &host.style, opts.cx.vcx(),
        );
        for first_height in [30., 50., 30.] {
            let Node::Element(first) = &mut host.children[1] else { panic!("normal sibling") };
            first.style.used_mut().reset_box_layout.height = Some(Len::Px(first_height));
            let measured = crate::layout::multicol::measure::plan_at(
                &host, &merged, &opts, (Some(100.), Some(100.)), (false, false),
                (Some(70.), Some(70.)), None, None,
            );
            assert_eq!(measured.source.len(), 6);
            assert_eq!(measured.stackable.as_ref().unwrap().len(), 2);
            assert_eq!(measured.oof_static.iter().map(|(at, _)| *at).collect::<Vec<_>>(), [0, 1, 2, 2]);
            let geometry = crate::layout::positioned::containing_block::metrics::layout::collect(
                &measured.source, &merged, 100., &opts, window, app,
            );
            assert_eq!(geometry.flows.len(), 4);
            assert_eq!(geometry.flows[1].metrics.static_rect.unwrap().origin.y, px(first_height));
            let recipe = Recipe::new(&host, &merged, &opts, 2, Some(70.), StackAxis::Horizontal, None).unwrap();
            let mut stack = ColumnStack::new(Vec::new(), 2, 10., Some(70.), None, None, None, Default::default());
            recipe.prepare(&mut stack, 100., window, app);
            assert_eq!(stack.children.len(), 6);
            let (_, lines, normal, _) = stack.balance(&stack.kid_geoms());
            assert_eq!(normal.completion, Completion::Complete);
            assert_eq!(normal.failure, None);
            *stack.lines_plan.borrow_mut() = lines;
            let placed = normal.iter().filter_map(|f| {
                let id = stack.children[f.kid].source?;
                Some((id, f.col, f.y, f.h))
            }).collect::<Vec<_>>();
            let ids: Vec<_> = host.children.iter().filter_map(|n| match n {
                Node::Element(c) if c.tag == "div" => Some(c.node_id), _ => None,
            }).collect();
            assert_eq!(placed, [(ids[0], 0, 0., first_height), (ids[1], 1, 0., 20.)]);
            stack.plan_oof(&normal, Bounds { origin: point(px(0.), px(0.)), size: size(px(210.), px(70.)) });
            let oof = stack.oof.plan.borrow();
            assert_eq!(oof.failure, None);
            assert_eq!(oof.completion, Completion::Complete);
            for (source, expected) in stack.oof.geometry.flows.iter().zip([
                (0, 0., 0., 85.), (0, first_height, 0., 85.),
                (1, 20., 0., 85.), (0, 7., 0., 85.),
            ]) {
                let fragments: Vec<_> = oof.iter().filter(|f| f.kid == source.id as usize)
                    .map(|f| (f.col, f.y, f.from, f.h)).collect();
                assert_eq!(fragments, [expected]);
            }
        }
        gpui::div()
    });
}
