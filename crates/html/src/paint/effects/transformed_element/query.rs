//! Запрос CSSOM: исходная раскладка до среза, точная проекция углов после среза.

use super::Transformed;
use crate::hooks::Hooks;
use crate::style::computed::{Transform, mul4};
use gpui::{App, Bounds, LayoutId, Pixels, Window};

pub(crate) struct QueryTransformed(pub(crate) Transformed);

impl Hooks for QueryTransformed {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, LayoutId) {
        let id = self.0.child.request_layout(window, cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _: Bounds<Pixels>,
        id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let group = &mut self.0;
        let bounds = Bounds::new(
            window.layout_origin_unrounded(*id),
            window.layout_size_unrounded(*id),
        );
        // Native continuation destinations already carry the root transform.
        let presented = group
            .node
            .is_some_and(|node| crate::paint::coordinator::segments::presented_root(node, window));
        let reference = group
            .ref_box
            .as_ref()
            .and_then(|r| r.get())
            .unwrap_or(bounds);
        let (w, h) = (
            f32::from(reference.size.width),
            f32::from(reference.size.height),
        );
        let (x, y) = (f32::from(reference.origin.x), f32::from(reference.origin.y));
        let origin = (
            x + w * group.origin.0 + group.origin_px.0.unwrap_or(0.),
            y + h * group.origin.1 + group.origin_px.1.unwrap_or(0.),
            group.origin_z.unwrap_or(0.),
        );
        let mut own = group.m4;
        for (i, row) in own.iter_mut().enumerate() {
            for (j, value) in row.iter_mut().enumerate() {
                *value += w * group.m4_pct[0][i][j] + h * group.m4_pct[1][i][j];
            }
        }
        let own = about(own, origin);
        let flat = !group.has_3d && group.frame_3d.is_none() && group.under_3d.is_none();
        if flat && !presented {
            crate::layout::positioned::anchor::tf_push(
                [
                    [own[0][0], own[0][1], own[0][3]],
                    [own[1][0], own[1][1], own[1][3]],
                ],
                &group.layout.anchors,
            );
        }
        let perspective = group.perspective.map(|distance| {
            about(
                Transform::perspective4(distance),
                (
                    x + w * group.perspective_origin.0
                        + group.perspective_origin_px.0.unwrap_or(0.),
                    y + h * group.perspective_origin.1
                        + group.perspective_origin_px.1.unwrap_or(0.),
                    0.,
                ),
            )
        });
        let flatten = group.frame_3d.is_none();
        let mut children = |window: &mut Window| {
            if let Some(perspective) = perspective {
                window.with_layout_projection(
                    perspective,
                    false,
                    group.node.map(|id| (id, true)),
                    |window| {
                        group.child.prepaint(window, cx);
                    },
                );
            } else {
                group.child.prepaint(window, cx);
            }
        };
        if presented {
            children(window);
        } else {
            window.with_layout_projection(own, flatten, group.node.map(|id| (id, false)), children);
        }
        if flat && !presented {
            crate::layout::positioned::anchor::tf_pop(&group.layout.anchors);
        }
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

fn about(matrix: [[f32; 4]; 4], (x, y, z): (f32, f32, f32)) -> [[f32; 4]; 4] {
    mul4(
        mul4(Transform::translate4(x, y, z), matrix),
        Transform::translate4(-x, -y, -z),
    )
}
