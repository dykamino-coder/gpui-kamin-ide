//! Настоящие полосы Repeat сохраняют старую краску в CSS-слоте общего эффекта.

use super::*;
use crate::layout::fragment::{paint_source::SourceSlices, types::RepeatGeom};
use gpui::{AnyElement, AvailableSpace, Corners, IntoElement, Styled, canvas, fill, rgb};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

fn repeat() -> Repeat {
    let band = |color| -> AnyElement {
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                window.paint_quad(fill(bounds, rgb(color)));
            },
        )
        .w(px(30.))
        .h(px(60.))
        .into_any_element()
    };
    Repeat {
        head: Some((0., 5.)),
        foot: Some((30., 6.)),
        geom: RepeatGeom::default(),
        head_els: vec![band(0x00ff00)],
        foot_els: vec![band(0x00ffff), band(0x00ffff)],
    }
}

fn place(
    repeat: &mut Repeat,
    fragment: &Frag,
    origin: gpui::Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    repeat.place_bands(fragment, |el, at| {
        el.layout_as_root_at(
            origin + point(px(0.), px(at)),
            size(
                AvailableSpace::Definite(px(30.)),
                AvailableSpace::Definite(px(60.)),
            ),
            window,
            cx,
        );
        el.prepaint_at(point(px(0.), px(0.)), window, cx);
    });
}

#[test]
fn repeated_table_bands_match_old_painter_between_oof_siblings_under_opacity() {
    let mut app = gpui::TestAppContext::single();
    app.add_empty_window()
        .draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
            crate::hooks::paint_root(
                canvas(
                    |_, window, cx| {
                        let fragment = Frag {
                            kid: 0,
                            copy: 1,
                            col: 0,
                            y: 10.,
                            from: 20.,
                            h: 20.,
                            head: 5.,
                            foot: 6.,
                        };
                        let origin = point(px(10.), px(10.));
                        let placement = PaintSegment {
                            id: PaintSegmentId(1),
                            ordinal: 0,
                            source: point(px(0.), px(0.)),
                            destination: point(px(0.), px(0.)),
                        };
                        let slice = PaintSegmentSlice {
                            placement,
                            source_clip: rect(0., 5., 90., 31.),
                        };
                        let mut actual = repeat();
                        let mut expected = repeat();
                        place(&mut actual, &fragment, origin, window, cx);
                        place(&mut expected, &fragment, origin, window, cx);
                        (
                            Slot {
                                repeat: Some(actual),
                                bands: vec![Band {
                                    fragment,
                                    origin,
                                    slice,
                                    tail: None,
                                }],
                                width: 30.,
                                spill: px(10.),
                            },
                            expected,
                        )
                    },
                    |_, state, window, cx| {
                        let (mut slot, mut expected) = state;
                        let extent = rect(0., 0., 100., 100.);
                        let part = slot.bands[0].slice;
                        let routes: Rc<[PaintSegment]> = Rc::from([part.placement]);
                        let slot = Rc::new(RefCell::new(Slot {
                            repeat: slot.repeat.take(),
                            bands: std::mem::take(&mut slot.bands),
                            width: slot.width,
                            spill: slot.spill,
                        }));
                        let mut slices = SourceSlices::default();
                        for id in [1, 2, 3] {
                            slices.nodes.insert(id, Rc::from([part]));
                        }
                        slices.slots.insert(
                            8,
                            Rc::new(move |window, cx| slot.borrow_mut().paint(window, cx)),
                        );
                        let sibling = |color, window: &mut Window| {
                            window.paint_quad(fill(rect(10., 5., 30., 31.), rgb(color)))
                        };
                        let actual = crate::hooks::paint_source_record(
                            extent,
                            routes.clone(),
                            slices,
                            window,
                            cx,
                            |window, cx| {
                                crate::hooks::paint_source_node(1, None, window, |window| {
                                    window.paint_group(
                                        extent,
                                        Corners::default(),
                                        0.,
                                        0.5,
                                        0,
                                        &[],
                                        None,
                                        None,
                                        |window| {
                                            crate::hooks::paint_source_node(
                                                2,
                                                None,
                                                window,
                                                |window| sibling(0xff0000, window),
                                            );
                                            crate::hooks::paint_source_slot(8, window, cx);
                                            crate::hooks::paint_source_node(
                                                3,
                                                None,
                                                window,
                                                |window| sibling(0x0000ff, window),
                                            );
                                        },
                                    );
                                });
                            },
                        )
                        .unwrap();
                        let (_, expected) = window
                            .record_paint_segments(extent, routes, cx, |window, cx| {
                                window.paint_group(
                                    extent,
                                    Corners::default(),
                                    0.,
                                    0.5,
                                    0,
                                    &[],
                                    None,
                                    None,
                                    |window| {
                                        window
                                            .with_paint_segment_slices(Rc::from([part]), |window| {
                                                sibling(0xff0000, window);
                                                expected.paint_bands(
                                                    &Frag {
                                                        kid: 0,
                                                        copy: 1,
                                                        col: 0,
                                                        y: 10.,
                                                        from: 20.,
                                                        h: 20.,
                                                        head: 5.,
                                                        foot: 6.,
                                                    },
                                                    |el, top, height| {
                                                        let mask = fragment_mask::snap(
                                                            rect(0., 10. + top, 50., height),
                                                            window.scale_factor(),
                                                        );
                                                        window.with_content_mask(
                                                            Some(mask),
                                                            |window| el.paint(window, cx),
                                                        );
                                                    },
                                                );
                                                sibling(0x0000ff, window);
                                            })
                                            .unwrap();
                                    },
                                );
                            })
                            .unwrap();
                        let a = actual.get(PaintSegmentId(1)).unwrap().scene();
                        let b = expected.get(PaintSegmentId(1)).unwrap().scene();
                        assert_eq!((a.groups.len(), a.surfaces.len()), (1, 1));
                        assert_eq!(a.surfaces[0].opacity, 0.5);
                        assert_eq!(a.groups[0].scene.quads.len(), 4);
                        assert_eq!(
                            format!("{:?}", a.groups[0].scene.quads),
                            format!("{:?}", b.groups[0].scene.quads)
                        );
                    },
                )
                .into_any_element(),
            )
        });
    app.quit();
}
