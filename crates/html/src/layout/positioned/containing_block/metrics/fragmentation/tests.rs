//! Исходный layout записывает собственные внутренние границы OOF.

use super::*;
use gpui::{point, px, size};

#[gpui::test]
fn oof_owns_measured_breaks_without_propagating_its_root_break(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let tree = crate::dom::parse("<section style='position:relative;width:100px;height:20px'><div style='position:absolute;top:65px;width:80px;break-before:column'><div style='height:30px'></div><div style='height:40px;break-before:column'></div></div><p style='height:20px;margin:0'></p></section>", "");
        let mut opts = RenderOpts { viewport: (400., 400.), normal_line_height: 1.2, text: Default::default(), cx: Default::default(), frag: Default::default() };
        opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
        opts.cx.values.viewport = opts.viewport;
        crate::style::cascade::materialize::materialize(&tree, opts.root_style().style_rc(), opts.cx.vcx());
        let mut pending: Vec<_> = tree.iter().filter_map(|n| match n { Node::Element(e) => Some(e), _ => None }).collect();
        let host = loop {
            let e = pending.pop().unwrap();
            if e.tag == "section" { break e }
            pending.extend(e.children.iter().filter_map(|n| match n { Node::Element(e) => Some(e), _ => None }));
        };
        let snapshot = super::super::layout::collect(std::slice::from_ref(host), &opts.root_style(), 100., &opts, window, app);
        assert_eq!(snapshot.flows.len(), 1);
        let source = &snapshot.flows[0];
        assert_eq!(source.owner, host.node_id);
        assert_eq!(source.kid.h, 70.);
        assert_eq!(source.metrics.normal_offset.y, px(65.));
        assert_eq!(source.kid.forced, [30.]);
        assert!(!source.kid.force_before);
        assert_eq!(source.owner_rect.size.height, px(20.));
        gpui::div()
    });
}
