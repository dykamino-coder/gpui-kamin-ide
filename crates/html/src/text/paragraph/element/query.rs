//! Фрагменты строчных коробок читает только явно запрошенная раскладка.

use crate::{
    hooks::{Hooks, IntoHooked},
    layout::geometry::Collector,
    text::paragraph::*,
};
use gpui::{App, Bounds, Element, Hitbox, LayoutId, Pixels, Window, point, px, size};

mod upright;

struct Query(Paragraph);

impl Paragraph {
    pub(crate) fn geometry_query(self) -> gpui::AnyElement {
        Query(self).hooked()
    }

    fn collect_geometry(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        if self.vertical {
            let bounds = if self.lines_reversed && !self.vertical_ccw {
                Bounds::new(
                    bounds.origin,
                    size(self.vertical_line_extent(), bounds.size.height),
                )
            } else {
                bounds
            };
            let flat = Bounds::new(bounds.origin, size(bounds.size.height, bounds.size.width));
            window.with_hitbox_transform(query_turn(bounds, self.vertical_ccw), |window| {
                self.collect_flat_geometry(flat, window)
            });
        } else if let Some(frame) = self.paint_context.frame() {
            let b = frame.bounds;
            let (corner, rotation_scale) = if frame.ccw {
                (point(b.origin.x, b.bottom()), [[0., 1.], [-1., 0.]])
            } else {
                (point(b.right(), b.origin.y), [[0., -1.], [1., 0.]])
            };
            let matrix = gpui::TransformationMatrix::unit()
                .translate(point(corner.x.scale(1.), corner.y.scale(1.)))
                .compose(gpui::TransformationMatrix {
                    rotation_scale,
                    translation: [0., 0.],
                })
                .translate(point(
                    (-b.origin.x).scale(1.),
                    (-b.origin.y - frame.phase).scale(1.),
                ));
            window
                .with_hitbox_transform(matrix, |window| self.collect_flat_geometry(bounds, window));
        } else {
            self.collect_flat_geometry(bounds, window);
        }
    }

    fn collect_flat_geometry(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let Some((values, _)) = &self.anchor_values else {
            return;
        };
        let segs = self.measure(window);
        for (at, _, overlay) in &self.overlays {
            let (Some(b), Some(info)) = (overlay.inline_box, overlay.inline_box_info.as_ref())
            else {
                continue;
            };
            let start = (*at as isize + b.start).max(0) as usize;
            let end = (*at as isize + b.end).max(0) as usize;
            if info.font_style.reset_context.upright_stack {
                self.collect_upright_geometry(&segs, start..end, info, b.shift, bounds, window);
                continue;
            }
            let edges = std::array::from_fn(|i| info.pad[i] + info.border[i]);
            let metrics = self.inline_font_metrics(info, window);
            let font = crate::layout::geometry::font(
                &info.font_style,
                f32::from(self.font_size),
                values.vcx(),
            );
            self.visit_inline_fragments(
                &segs,
                start..end,
                edges,
                metrics,
                bounds,
                |mut border, present, _| {
                    border.origin += point(px(b.shift.0), px(b.shift.1));
                    let inset = |r: Bounds<Pixels>, e: [f32; 4]| {
                        Bounds::new(
                            r.origin + point(px(e[3]), px(e[0])),
                            size(
                                (r.size.width - px(e[1] + e[3])).max(px(0.)),
                                (r.size.height - px(e[0] + e[2])).max(px(0.)),
                            ),
                        )
                    };
                    let padding = inset(
                        border,
                        std::array::from_fn(|i| if present[i] { info.border[i] } else { 0. }),
                    );
                    let content = inset(
                        padding,
                        std::array::from_fn(|i| if present[i] { info.pad[i] } else { 0. }),
                    );
                    Collector::record(window, info.node, [border, padding, content], font);
                },
            );
        }
    }
}

impl Hooks for Query {
    type Layout = LayoutId;
    type Prepaint = Option<Hitbox>;
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, LayoutId) {
        self.0.request_layout(None, None, window, cx)
    }
    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        let result = self.0.prepaint(None, None, bounds, layout, window, cx);
        let exact = Bounds::new(
            window.layout_origin_unrounded(*layout),
            window.layout_size_unrounded(*layout),
        );
        self.0.collect_geometry(exact, window);
        result
    }
    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout: &mut LayoutId,
        prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0
            .paint(None, None, bounds, layout, prepaint, window, cx);
    }
}

fn query_turn(b: Bounds<Pixels>, ccw: bool) -> gpui::TransformationMatrix {
    let (corner, rotation_scale) = if ccw {
        (point(b.left(), b.bottom()), [[0., 1.], [-1., 0.]])
    } else {
        (point(b.right(), b.top()), [[0., -1.], [1., 0.]])
    };
    gpui::TransformationMatrix::unit()
        .translate(point(corner.x.scale(1.), corner.y.scale(1.)))
        .compose(gpui::TransformationMatrix {
            rotation_scale,
            translation: [0., 0.],
        })
        .translate(point((-b.left()).scale(1.), (-b.top()).scale(1.)))
}
