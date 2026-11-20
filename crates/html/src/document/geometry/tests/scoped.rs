//! Synchronous CSSOM reads within one layout of the host window.

use super::*;

#[gpui::test]
fn adjacent_cssom_reads_share_layout_and_match_fresh_fragments(cx: &mut TestAppContext) {
    let html = "<style>body{margin:0}#box{width:38px;height:15px;padding:2px}</style><p>before</p><!-- gap --><div id=box></div><i id=empty style='display:none'></i>";
    let expected = snapshot(cx, html, 300.);
    let target = find(&expected, "box");
    assert!(target.border_box.width > 0.);
    let hidden = find(&expected, "empty");
    let doc = Document::builder(html, BROWSER_CSS).build().unwrap();
    let (view, cx) = cx.add_window_view(|_, _| View(Rc::new(doc)));
    cx.update(|window, cx| {
        let opts = RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: (300., 300.),
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        let doc = view.read(cx).0.clone();
        let mut source = doc.source_dom();
        for index in &target.path {
            let child = source.children.borrow()[*index].clone();
            source = child;
        }
        let before = doc.state.geometry_flushes.get();
        doc.with_geometry(
            crate::GeometryLayout::Continuous,
            None,
            &opts,
            window,
            cx,
            |read| {
                assert_eq!(read.client_rects_for(&source), target.client_rects);
                assert_eq!(read.get_client_rects(&target.path), target.client_rects);
                assert_eq!(
                    read.get_bounding_client_rect(&target.path),
                    target.border_box
                );
                assert!(read.get_client_rects(&hidden.path).is_empty());
                assert_eq!(
                    read.get_bounding_client_rect(&[usize::MAX]),
                    crate::Rect::default()
                );
            },
        )
        .unwrap();
        assert_eq!(doc.state.geometry_flushes.get() - before, 1);
    });
}
