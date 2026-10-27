//! Рекордер OOF принимает собственные строки и изолирует вложенный независимый поток.

use super::*;
use crate::layout::fragment::types::StackAxis;
use crate::layout::fragment::{ShapeCx, flow::blocks::BlockBox, paginate::watch::Watched};
use gpui::{AvailableSpace, IntoElement, ParentElement, Styled, div, point, px, size};

#[gpui::test]
fn independent_oof_root_records_only_its_own_lines(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let tree = crate::dom::parse("<section style='position:absolute;width:100px;font-size:10px;line-height:20px'>a<br>b<br>c<div style='position:absolute;top:100px;width:50px'>x<br>y</div></section>", "");
        let mut opts = RenderOpts {
            viewport: (400., 400.), normal_line_height: 1.2,
            text: Default::default(), cx: Default::default(), frag: Default::default(),
        };
        opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
        opts.cx.values.viewport = opts.viewport;
        crate::style::cascade::materialize::materialize(&tree, opts.root_style().style_rc(), opts.cx.vcx());
        let mut pending: Vec<_> = tree.iter().collect();
        let owner = loop {
            let Some(node) = pending.pop() else { panic!("section owner") };
            if let crate::dom::Node::Element(e) = node {
                if e.tag == "section" { break e; }
                pending.extend(&e.children);
            }
        };
        let blocks = BlockBox::collect(owner, ShapeCx::COLUMNS, opts.cx.vcx());
        let watch = Watched::for_blocks_axis(blocks, StackAxis::Horizontal, true);
        opts.frag.watch = Some(watch.clone());
        let mut root = div().block().w(px(100.)).child(element(owner, &opts.root_style(), &opts)).into_any_element();
        root.layout_as_root(size(AvailableSpace::Definite(px(100.)), AvailableSpace::MaxContent), window, app);
        let bounds = watch.normal(owner.node_id, window).expect("normal OOF box");
        let geometry = watch.flow.at(bounds.origin, bounds.size, StackAxis::Horizontal, window);
        let lines: Vec<_> = geometry.inline.iter().flat_map(|p| &p.lines).map(|l| (l.top, l.bottom)).collect();
        assert_eq!(lines, [(0., 20.), (20., 40.), (40., 60.)]);
        div()
    });
}
