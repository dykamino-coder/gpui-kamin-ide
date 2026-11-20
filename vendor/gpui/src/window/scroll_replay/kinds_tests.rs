//! Every primitive an element paints under a scroll node is marked: a path
//! (`paint_path`) and an image (`paint_image_with_sampling`) replay scrolled
//! like a quad, and an unmarked insert names its refusal.

use super::*;
use crate::{ImageSampling, Path, Quad, RenderImage, ScrollHandle, TestAppContext, div, px, size};
use image::{Frame, ImageBuffer, Rgba};

struct Empty;

impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

/// Record `paint` under one scroll node, then move the node by 10 px and
/// plan the scrolled replay of the record.
fn plan(cx: &mut TestAppContext, paint: impl Fn(&mut Window)) -> Result<(), ScrollRefusal> {
    let window = cx.add_window(|_, _| Empty);
    window
        .update(cx, |_, window, _| {
            window.scroll_interest_margin = px(100.);
            let handle = ScrollHandle::new();
            handle.set_max_offset(point(px(0.), px(1000.)));
            window.invalidator.set_phase(DrawPhase::Prepaint);
            let (_, prepaint) = window.record_retained_prepaint(|window| {
                window.with_scroll_node_prepaint(&handle, rect(0., 0., 100., 50.), |_| {});
            });
            window.invalidator.set_phase(DrawPhase::Paint);
            let (_, record) = window.record_retained_paint(|window| {
                window.with_scroll_node_paint(ScrollNodeId::of(&handle), |window| paint(window));
            });
            std::mem::swap(&mut window.rendered_frame, &mut window.next_frame);
            window.next_frame.clear();
            handle.set_offset(point(px(0.), px(-10.)));
            window.scroll_replay_plan(&prepaint, &record, None).map(|_| ())
        })
        .unwrap()
}

#[crate::test]
fn paths_and_images_under_a_node_replay_scrolled(cx: &mut TestAppContext) {
    let path = |window: &mut Window| {
        let mut path = Path::new(point(px(0.), px(10.)));
        path.line_to(point(px(20.), px(10.)));
        path.line_to(point(px(10.), px(30.)));
        window.paint_path(path, crate::black());
    };
    assert_eq!(plan(cx, path), Ok(()), "a path");
    let image = |window: &mut Window| {
        let frame = Frame::new(ImageBuffer::from_pixel(4, 4, Rgba([0, 0, 0, 255])));
        let data = Arc::new(RenderImage::new(smallvec::SmallVec::from_elem(frame, 1)));
        let sampling = ImageSampling::LinearSubpixel;
        window.paint_image_with_sampling(rect(0., 10., 4., 4.), Default::default(), data, 0, false, sampling).unwrap();
    };
    assert_eq!(plan(cx, image), Ok(()), "an image");
    let unmarked = |window: &mut Window| {
        let bounds = rect(0., 10., 10., 10.).scale(window.scale_factor());
        window.next_frame.scene.insert_primitive(Quad { bounds, content_mask: ContentMask { bounds }, ..Default::default() });
    };
    assert_eq!(plan(cx, unmarked), Err(ScrollRefusal::Unmarked), "a quad inserted past the node");
}
