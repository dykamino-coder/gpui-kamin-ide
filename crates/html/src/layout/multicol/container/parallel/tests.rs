//! Рост фрагментов сохраняет исходное положение и размер содержащего блока.

use super::*;
use crate::style::computed::Computed;

fn element(style: Computed, children: Vec<Node>) -> Element {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(10_000);
    Element {
        marks: Default::default(),
        node_id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        tag: "div".into(),
        style: style.into(),
        children,
        attrs: Vec::new(),
        inline: false,
        list_item: None,
        anim: None,
        hover: None,
        first_letter: None,
        first_line: None,
    }
}

fn containing_block() -> Element {
    let children = [(90.0, false), (50.0, true), (50.0, false)]
        .into_iter()
        .map(|(h, avoid)| {
            Node::Element(element(
                Computed {
                    height: Some(Len::Px(h)),
                    break_inside_avoid: avoid,
                    ..Default::default()
                },
                Vec::new(),
            ))
        })
        .collect();
    let mut abs = element(
        Computed {
            position: Some(Position::Absolute),
            ..Default::default()
        },
        children,
    );
    abs.style.used_mut().inset.bottom = Some(Len::Px(0.0));
    element(
        Computed {
            height: Some(Len::Px(190.0)),
            position: Some(Position::Relative),
            ..Default::default()
        },
        vec![Node::Element(abs)],
    )
}

#[gpui::test]
fn bottom_position_is_resolved_before_auto_height_grows(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        gpui::point(gpui::px(0.), gpui::px(0.)),
        gpui::size(gpui::px(400.), gpui::px(400.)),
        |window, app| {
            let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
            let vcx = doc.values();
            let mut cb = containing_block();
            let geometry = original(&cb, window, app);
            assert!(grow_positioned_descendants(
                &mut cb, &geometry, 100.0, 2, 4, vcx
            ));
            let Node::Element(abs) = &cb.children[0] else {
                panic!()
            };
            assert_eq!(abs.style.used().inset.top, Some(Len::Px(0.0)));
            assert_eq!(
                measure_fragment_and_own_height(abs, vcx).unwrap().0.height,
                200.0
            );
            let (shape, own) = measure_fragment_and_own_height(&cb, vcx).unwrap();
            assert_eq!(shape.height, 200.0);
            assert_eq!(own, 190.0);
            gpui::div()
        },
    );
}

#[gpui::test]
fn independent_breaks_use_the_containing_blocks_fragmentainer_phase(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        gpui::point(gpui::px(0.), gpui::px(0.)),
        gpui::size(gpui::px(400.), gpui::px(400.)),
        |window, app| {
            let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
            let vcx = doc.values();
            let mut cb = containing_block();
            cb.style.used_mut().margin.top = Some(Len::Px(80.));
            cb.style.used_mut().inset.top = Some(Len::Px(33.));
            let geometry = original(&cb, window, app);
            assert!(grow_positioned_descendants(
                &mut cb, &geometry, 100.0, 2, 4, vcx
            ));
            let Node::Element(abs) = &cb.children[0] else {
                panic!()
            };
            assert_eq!(abs.style.used().inset.top, Some(Len::Px(0.0)));
            assert_eq!(
                measure_fragment_and_own_height(abs, vcx).unwrap().0.height,
                220.0
            );
            assert_eq!(measure_fragment_and_own_height(&cb, vcx).unwrap().1, 190.0);
            gpui::div()
        },
    );
}

fn original(cb: &Element, window: &mut gpui::Window, app: &mut gpui::App) -> std::rc::Rc<Snapshot> {
    let opts = crate::layout::options::RenderOpts {
        viewport: (400., 400.),
        normal_line_height: 1.2,
        text: Default::default(),
        cx: Default::default(),
        frag: Default::default(),
    };
    crate::layout::positioned::containing_block::metrics::layout::collect(
        std::slice::from_ref(cb),
        &opts.root_style(),
        200.,
        &opts,
        window,
        app,
    )
}
