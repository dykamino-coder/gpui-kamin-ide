//! Постраничный запрос: реальные продолжения блока и сохранённые коробки DOM.

use super::*;

#[gpui::test]
fn geometry_paged_block_has_page_fragments(cx: &mut TestAppContext) {
    let doc = Document::builder(
        "<style>html,body{margin:0}</style><div id='split' style='height:180px;width:50px;background:green'></div>",
        BROWSER_CSS,
    )
    .build()
    .unwrap();
    let (view, cx) = cx.add_window_view(|_, _| View(Rc::new(doc)));
    cx.update(|window, cx| {
        let opts = RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: (200., 300.),
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        let pages = Rc::new(|_, _: &str| crate::flow::PageGeom {
            size: (100., 100.),
            area: (100., 100.),
            margin: [0.; 4],
            border: (0., gpui::black()),
            outline: (0., 0., gpui::black()),
            padding: [0.; 4],
            bg: gpui::white(),
            canvas: None,
            turn: 0,
        });
        let elements = view
            .read(cx)
            .0
            .clone()
            .element_geometry_with_layout(
                crate::GeometryLayout::Paged {
                    pages,
                    margins: None,
                },
                None,
                &opts,
                window,
                cx,
            )
            .unwrap();
        let split = find(&elements, "split");
        assert_eq!(split.client_rects.len(), 2, "{:?}", split.client_rects);
        assert!(split.client_rects.iter().all(|r| r.width > 0.));
        assert!(
            (split.client_rects[0].height - 100.).abs() < 0.001,
            "{:?}",
            split.client_rects
        );
        assert!(
            (split.client_rects[1].height - 80.).abs() < 0.001,
            "{:?}",
            split.client_rects
        );
        assert_ne!(
            (split.client_rects[0].x, split.client_rects[0].y),
            (split.client_rects[1].x, split.client_rects[1].y)
        );
        for tag in ["html", "body"] {
            let root = elements.iter().find(|e| e.tag == tag).unwrap();
            assert!(!root.client_rects.is_empty(), "{tag}");
        }
    });
}
