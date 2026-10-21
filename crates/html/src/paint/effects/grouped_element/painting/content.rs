//! Покраска содержимого группы в буфер: при `corner_clip` собственные декорации коробки
//! компонуются отдельно от фигурной обрезки переполнения вложенной группой, иначе одна группа
//! с полигоном и маской; ребёнок красится в своей фазе контуров, кольцо `corner-shape`
//! замещаемого элемента — поверх содержимого в пределах content-box.
use crate::paint::effects::grouped_element::Grouped;
use gpui::{App, Bounds, Corners, Pixels, Point, RenderImage, Window, px};
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    group: &mut Grouped,
    border_box: Bounds<Pixels>,
    cx: &mut App,
    area: Bounds<Pixels>,
    corners: Corners<Pixels>,
    polygon: &[Point<Pixels>],
    mask: Option<(Arc<RenderImage>, Bounds<Pixels>, u32)>,
    mask_clip: Option<[f32; 4]>,
    window: &mut Window,
) {
    let (blur, opacity, blend) = (group.blur, group.opacity, group.blend);
    if group.corner_clip.is_some() {
        window.paint_group(
            area,
            corners,
            blur,
            opacity,
            blend,
            &[],
            None,
            mask_clip,
            |window| {
                if let Some(clip) = group.corner_clip.as_mut() {
                    clip.decoration.paint(window, cx);
                }
                window.paint_group(
                    area,
                    Corners::default(),
                    0.0,
                    1.0,
                    0,
                    polygon,
                    mask,
                    None,
                    |w| {
                        child(group, border_box, w, cx);
                    },
                );
            },
        );
    } else {
        window.paint_group(
            area,
            corners,
            blur,
            opacity,
            blend,
            polygon,
            mask,
            mask_clip,
            |w| {
                child(group, border_box, w, cx);
            },
        );
    }
}

fn child(group: &mut Grouped, border_box: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let paint = group.values.vcx().paint.cloned();
    crate::paint::decorations::outline_phase::scoped(paint.as_ref(), window, cx, |window, cx| {
        group.child.paint(window, cx);
    });
    if let Some((spec, colour, [t, r, b, l])) = &group.ring_over {
        let content = Bounds {
            origin: gpui::point(border_box.origin.x + px(*l), border_box.origin.y + px(*t)),
            size: gpui::size(
                border_box.size.width - px(l + r),
                border_box.size.height - px(t + b),
            ),
        };
        let content = if colour.a >= 1.0 { border_box } else { content };
        window.with_content_mask(Some(gpui::ContentMask { bounds: content }), |w| {
            crate::paint::decorations::shaped_ring::paint(spec, *colour, border_box, w)
        });
    }
}
