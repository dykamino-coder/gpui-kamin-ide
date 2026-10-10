//! Resolve basic rectangular clips before snapping their absolute device edges.
use crate::paint::effects::grouped_element::Grouped;
use crate::paint::effects::{legacy_clip, polygon_clip};
use gpui::{Bounds, LayoutId, Pixels, Window};

fn rectangular(group: &Grouped) -> bool {
    group.mask_clip_off.is_none()
        && (group.clip_inset.is_some()
            || group.clip_edges.is_some()
            || group.clip_xywh.is_some()
            || group.clip_rect.is_some())
}

pub(super) fn reference_box(
    group: &Grouped,
    fallback: Bounds<Pixels>,
    id: LayoutId,
    window: &mut Window,
) -> Bounds<Pixels> {
    let snap_rectilinear = window.current_transformation() == gpui::TransformationMatrix::unit();
    let mask_clip = group.mask_clip_off.is_some() && snap_rectilinear;
    if mask_clip
        || rectangular(group)
        || group.polygon.len() == 4
        || (snap_rectilinear && group.polygon.len() >= 4)
    {
        // CSS Shapes §3.1: percentages use the reference box, not its raster
        // bounds. Blink clip_path_clipper.cc:383 likewise retains layout geometry.
        let reference = Bounds {
            origin: window.layout_origin_unrounded(id),
            size: window.layout_size_unrounded(id),
        };
        let points = polygon_clip::points(group, reference);
        if mask_clip
            || rectangular(group)
            || polygon_clip::rectangle(&points, 1.0).is_some()
            || (snap_rectilinear && polygon_clip::rectilinear(&points))
        {
            reference
        } else {
            fallback
        }
    } else {
        fallback
    }
}

pub(super) fn device_edges(group: &Grouped, rect: [f32; 4]) -> [f32; 4] {
    if !rectangular(group) && group.mask_clip_off.is_none() {
        return rect;
    }
    // CSS Masking 1 section 7.5 uses the same painting boxes as
    // background-clip. Snap their final device edges like background paint,
    // rather than excluding the last pixel at a half-device-pixel endpoint.
    legacy_clip::snap(rect)
}

pub(super) fn resolve(group: &Grouped, clip_bounds: Bounds<Pixels>, sf: f32) -> Option<[f32; 4]> {
    group
        .mask_clip_off
        .map(|[ct, cr, cb, cl]| {
            [
                cl,
                ct,
                (f32::from(clip_bounds.size.width) - cl - cr).max(0.0),
                (f32::from(clip_bounds.size.height) - ct - cb).max(0.0),
            ]
        })
        .or_else(|| {
            // `clip-path: inset(...)` — срезы краёв (css-shapes-1).
            group.clip_inset.map(|[t, r, b, l]| {
                let (bw, bh) = (
                    f32::from(clip_bounds.size.width),
                    f32::from(clip_bounds.size.height),
                );
                let side = |v: crate::style::values::value::Len, s: f32| match v {
                    crate::style::values::value::Len::Px(p) => p,
                    crate::style::values::value::Len::Pct(p) => p * s,
                    _ => 0.0,
                };
                let (t, b) = (side(t, bh), side(b, bh));
                let (l, r) = (side(l, bw), side(r, bw));
                [l, t, (bw - l - r).max(0.0), (bh - t - b).max(0.0)]
            })
        })
        .or_else(|| {
            // `clip-path: rect(...)` — края от верхнего-левого угла.
            group.clip_edges.map(|[t, r, b, l]| {
                let (bw, bh) = (
                    f32::from(clip_bounds.size.width),
                    f32::from(clip_bounds.size.height),
                );
                let side = |v: Option<crate::style::values::value::Len>, s: f32, def: f32| match v {
                    Some(crate::style::values::value::Len::Px(p)) => p,
                    Some(crate::style::values::value::Len::Pct(p)) => p * s,
                    _ => def,
                };
                let (t, b) = (side(t, bh, 0.0), side(b, bh, bh));
                let (l, r) = (side(l, bw, 0.0), side(r, bw, bw));
                // css-shapes-1 `rect()`: правый край левее левого (нижний
                // выше верхнего) зажимается до него — область ПУСТА, элемент
                // скрыт целиком. Шейдер композита нулевую ширину читает как
                // «клипа нет» (`shaders.hlsl`: `mask_clip.z > 0.0`), поэтому
                // коробка уводится за экран — тот же приём, что у `clip:
                // rect()` ниже (clip-path-rect-004: `rect(50px 0 0 50px)`,
                // красная коробка 2.08).
                if r < l || b < t {
                    return [-1.0e7, -1.0e7, 1.0, 1.0];
                }
                [l, t, r - l, b - t]
            })
        })
        .or_else(|| {
            // `clip-path: xywh(x y w h)` — прямоугольник от угла.
            group.clip_xywh.map(|[x, y, w, h]| {
                let (bw, bh) = (
                    f32::from(clip_bounds.size.width),
                    f32::from(clip_bounds.size.height),
                );
                let side = |v: crate::style::values::value::Len, s: f32| match v {
                    crate::style::values::value::Len::Px(p) => p,
                    crate::style::values::value::Len::Pct(p) => p * s,
                    _ => 0.0,
                };
                [
                    side(x, bw),
                    side(y, bh),
                    side(w, bw).max(0.0),
                    side(h, bh).max(0.0),
                ]
            })
        })
        .or_else(|| {
            group.clip_rect.map(|edges| {
                // CSS 2.1 §11.1.2: auto refers to the unrounded border-box edge.
                legacy_clip::resolve(
                    edges,
                    f32::from(clip_bounds.size.width),
                    f32::from(clip_bounds.size.height),
                )
            })
        })
        .map(|[x, y, w, h]| {
            let (dx, dy) = (
                group.clip_shift.0 + f32::from(clip_bounds.size.width) * group.clip_shift.2,
                group.clip_shift.1 + f32::from(clip_bounds.size.height) * group.clip_shift.3,
            );
            let rect = [
                (f32::from(clip_bounds.origin.x) + x + dx) * sf,
                (f32::from(clip_bounds.origin.y) + y + dy) * sf,
                w * sf,
                h * sf,
            ];
            device_edges(group, rect)
        })
}
