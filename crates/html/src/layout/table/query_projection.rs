//! Проекция виртуальной коробки применяет CSS-матрицу после раскладки дорожек.

use crate::style::computed::{Computed, Transform, mul4};
use gpui::{Bounds, Pixels, Window};

pub(super) fn enter<R>(
    style: &Computed,
    bounds: Bounds<Pixels>,
    children: bool,
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let origin = style
        .reset_transforms
        .transform_origin
        .unwrap_or((0.5, 0.5));
    let origin_px = style.reset_transforms.transform_origin_px;
    let (x, y, z) = (
        f32::from(bounds.origin.x) + w * origin.0 + origin_px.0.unwrap_or(0.),
        f32::from(bounds.origin.y) + h * origin.1 + origin_px.1.unwrap_or(0.),
        style.reset_transforms.transform_origin_z.unwrap_or(0.),
    );
    let matrix = style.reset_transforms.transform.map(|transform| {
        let transform = transform.after_individual(
            style.reset_transforms.rotate_prop,
            style.reset_transforms.scale_prop,
        );
        let mut matrix = transform.m4;
        for (i, row) in matrix.iter_mut().enumerate() {
            for (j, value) in row.iter_mut().enumerate() {
                *value += w * transform.m4_pct[0][i][j] + h * transform.m4_pct[1][i][j];
            }
        }
        about(matrix, (x, y, z))
    });
    let perspective = children
        .then_some(style.reset_transforms.perspective)
        .flatten()
        .map(|distance| {
            let origin = style
                .reset_transforms
                .perspective_origin
                .unwrap_or((0.5, 0.5));
            let delta = style.reset_transforms.perspective_origin_px;
            about(
                Transform::perspective4(distance),
                (
                    f32::from(bounds.origin.x) + w * origin.0 + delta.0.unwrap_or(0.),
                    f32::from(bounds.origin.y) + h * origin.1 + delta.1.unwrap_or(0.),
                    0.,
                ),
            )
        });
    let inside = |window: &mut Window| match perspective {
        Some(matrix) => window.with_layout_projection(matrix, false, None, f),
        None => f(window),
    };
    let flatten = style.reset_transforms.preserve_3d != Some(true) || style.flattens_3d();
    let identity = Transform::translate4(0., 0., 0.);
    window.with_layout_projection(matrix.unwrap_or(identity), flatten, None, inside)
}

fn about(matrix: [[f32; 4]; 4], (x, y, z): (f32, f32, f32)) -> [[f32; 4]; 4] {
    mul4(
        mul4(Transform::translate4(x, y, z), matrix),
        Transform::translate4(-x, -y, -z),
    )
}
