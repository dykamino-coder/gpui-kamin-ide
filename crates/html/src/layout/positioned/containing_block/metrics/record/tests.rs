//! Исходные CB/строка и зависимости завершаются вместе с normal layout.
use super::*;
use gpui::{AvailableSpace, Style};

fn sample(html: &str, width: f32, window: &mut Window, app: &mut App) -> (Rc<Snapshot>, u64) {
    let tree = crate::dom::parse(html, "");
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
        &tree,
        opts.root_style().style_rc(),
        opts.cx.vcx(),
    );
    fn section(nodes: &[crate::dom::Node]) -> Option<&crate::dom::Element> {
        nodes.iter().find_map(|n| match n {
            crate::dom::Node::Element(e) if e.tag == "section" => Some(e),
            crate::dom::Node::Element(e) => section(&e.children),
            _ => None,
        })
    }
    let cb = section(&tree).unwrap();
    let id = cb
        .children
        .iter()
        .find_map(|n| match n {
            crate::dom::Node::Element(e)
                if crate::layout::box_kind::out_of_flow(&e.style.used()) =>
            {
                Some(e.node_id)
            }
            _ => None,
        })
        .unwrap();
    (
        super::super::layout::collect(
            std::slice::from_ref(cb),
            &opts.root_style(),
            width,
            &opts,
            window,
            app,
        ),
        id,
    )
}

#[gpui::test]
fn padding_cb_bottom_and_relative_offset_are_recorded_before_growth(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let (snapshot, id) = sample("<section style='position:relative;top:77px;left:31px;width:120px;height:100px;padding:8px;border:4px solid'><div style='position:absolute;left:9px;bottom:6px;width:20px;height:30px'></div></section>", 200., window, app);
        let first = *snapshot.get(id).expect("resolved OOF");
        assert_eq!(first.original_cb_rect.origin, point(px(4.), px(4.)));
        assert_eq!(first.original_cb_rect.size, size(px(136.), px(116.)));
        assert_eq!(first.normal_offset, point(px(9.), px(80.)));
        assert_eq!(first.normal_size, size(px(20.), px(30.)));
        assert!(first.static_rect.is_none());
        let _ = sample("<section style='position:relative;height:250px'><div style='position:absolute;bottom:0;width:10px;height:10px'></div></section>", 180., window, app);
        assert_eq!(snapshot.get(id).unwrap().normal_rect(), first.normal_rect());
        gpui::div()
    });
}

#[gpui::test]
// Нулевой кегль исключает фиктивный advance U+FEFF тестового шейпера GPUI.
// Ширину гипотетического предшественника задаёт только inline-block.
fn static_position_uses_the_middle_of_the_actual_line(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let (snapshot, id) = sample("<section style='position:relative;font-size:0;top:70px;width:160px;height:100px;padding:8px;border:4px solid'><span style='display:inline-block;width:30px;height:20px'></span><span style='position:absolute;top:10px;width:10px;height:10px'></span></section>", 200., window, app);
        let metrics = snapshot.get(id).expect("static dependency completed");
        assert_eq!(metrics.normal_offset, point(px(38.), px(10.)));
        assert_eq!(metrics.static_rect.unwrap().origin.x - metrics.original_cb_rect.origin.x, px(38.));
        assert!(metrics.dependencies.static_position.is_some());
        assert_eq!(metrics.static_rect.unwrap().size.height, px(20.));
        gpui::div()
    });
}

#[gpui::test]
fn auto_height_cb_uses_in_flow_content_before_resolving_bottom(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let (snapshot, id) = sample("<section style='position:relative;top:99px;width:120px;padding:10px;border:4px solid'><div style='height:80px'></div><div style='position:absolute;left:9px;bottom:6px;width:20px;height:30px'></div></section>", 200., window, app);
        let metrics = snapshot.get(id).unwrap();
        assert_eq!(metrics.original_cb_rect.size, size(px(140.), px(100.)));
        assert_eq!(metrics.normal_offset, point(px(9.), px(64.)));
        gpui::div()
    });
}

#[gpui::test]
fn rtl_mid_line_static_position_uses_the_inline_end_edge(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let (snapshot, id) = sample("<section style='position:relative;font-size:0;direction:rtl;width:160px;height:100px;padding:8px;border:4px solid'><span style='display:inline-block;width:30px;height:20px'></span><span style='position:absolute;top:10px;width:10px;height:10px'></span></section>", 200., window, app);
        let metrics = snapshot.get(id).expect("RTL static dependency completed");
        assert_eq!(metrics.normal_offset, point(px(128.), px(10.)));
        assert_eq!(metrics.static_rect.unwrap().origin.x - metrics.original_cb_rect.origin.x, px(138.));
        gpui::div()
    });
}

#[gpui::test]
fn all_auto_inline_overlay_records_its_actual_static_point(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        for (direction, expected, bias) in [("ltr", 38., 0.), ("rtl", 128., 10.)] {
            let html = format!("<section style='position:relative;font-size:0;direction:{direction};width:160px;height:100px;padding:8px;border:4px solid'><span style='display:inline-block;width:30px;height:20px'></span><span style='position:absolute;width:10px;height:10px'></span></section>");
            let (snapshot, id) = sample(&html, 200., window, app);
            let metrics = snapshot.get(id).expect("all-auto static dependency completed");
            assert_eq!(metrics.normal_offset.x, px(expected));
            assert_eq!(metrics.normal_rect().origin.x + px(bias), metrics.static_rect.unwrap().origin.x);
            assert_eq!(metrics.normal_rect().origin.y, metrics.static_rect.unwrap().origin.y);
        }
        gpui::div()
    });
}

#[gpui::test]
fn recording_requires_final_membership_and_both_constraints(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(400.), px(400.)),
        |window, app| {
            let child = window.request_layout(Style::default(), [], app);
            let root = window.request_layout(Style::default(), [child], app);
            let recorder = Recorder::default();
            recorder.nodes.borrow_mut().insert(
                1,
                Entry {
                    node: Node::capture(root, window),
                    cb: 0,
                    border: [0.; 4],
                    positioned: false,
                },
            );
            recorder.nodes.borrow_mut().insert(
                2,
                Entry {
                    node: Node::capture(child, window),
                    cb: 1,
                    border: [0.; 4],
                    positioned: true,
                },
            );
            assert!(recorder.snapshot(window).get(2).is_none());
            let mut previous = None;
            for (w, h) in [(200., 100.), (200., 150.), (160., 150.)] {
                window.compute_layout(
                    root,
                    size(
                        AvailableSpace::Definite(px(w)),
                        AvailableSpace::Definite(px(h)),
                    ),
                    app,
                );
                let snapshot = recorder.snapshot(window);
                let metrics = snapshot.get(2).unwrap();
                if let Some(previous) = previous {
                    assert_ne!(metrics.dependencies, previous);
                }
                previous = Some(metrics.dependencies);
                let mut invalid = snapshot.clone();
                invalid.containing_blocks.clear();
                assert!(invalid.get(2).is_none());
            }
            window.set_flex_item_fragmentation_consumed(child, 1.);
            assert!(recorder.snapshot(window).get(2).is_none());
            gpui::div()
        },
    );
}
