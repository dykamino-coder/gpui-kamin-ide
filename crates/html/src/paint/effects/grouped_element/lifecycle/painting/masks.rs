//! Разрешение изображения маски группы и размещение её плитки.

mod composite;
pub(super) use composite::composite_mask;

mod layer;
pub(super) use layer::mask_layer;

use crate::paint::effects::grouped_element::{Grouped, mask_layer_source};
use crate::paint::effects::mask_size;
use gpui::{Bounds, Pixels, Window, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_mask(
    src: &str,
    group: &Grouped,
    bounds: Bounds<Pixels>,
    _prepaint: &(Bounds<Pixels>, Bounds<Pixels>),
    window: &mut Window,
    sl: f32,
    st: f32,
    sr: f32,
    sb: f32,
) -> Option<(std::sync::Arc<gpui::RenderImage>, Bounds<Pixels>, u32)> {
    // `border-shape` (css-borders-4): маска — внешний контур рамки,
    // SVG-растр на РАСШИРЕННУЮ область (обводка выходит за
    // border-box), одной плиткой без мощения; альфа = покрытие.
    if let Some(spec) = src.strip_prefix("bordershape:") {
        let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let (aw, ah) = (bw + sl + sr, bh + st + sb);
        let markup = crate::paint::background::border_shape_mask_svg(spec, bw, bh, sl, st, aw, ah)?;
        let img = crate::svg::raster::rasterize(&markup, aw, ah)?;
        return Some((
            img,
            Bounds {
                origin: gpui::point(bounds.origin.x - px(sl), bounds.origin.y - px(st)),
                size: gpui::size(px(aw), px(ah)),
            },
            // Одна плитка: за пределами области пусто.
            3,
        ));
    }
    // Слои: `url(a), url(b)` — полотно, собранное по mask-composite;
    // одиночный слой идёт плиткой прямо в композит.
    let layers: Vec<String> = if src.starts_with("shape:") {
        vec![src.to_string()]
    } else {
        crate::style::css::split_args(src)
            .iter()
            .filter_map(|l| mask_layer_source(l))
            .collect()
    };
    let src: &str = layers.first().map(String::as_str)?;
    let bounds = _prepaint.1;
    // Коробка укладки (`mask-origin`): плитка и её свободное место
    // считаются от неё, а не от border-box.
    let [ot, or_, ob, ol] = group.mask_origin_off;
    let (bw, bh) = (
        f32::from(bounds.size.width) - ol - or_,
        f32::from(bounds.size.height) - ot - ob,
    );
    let len = |l: crate::style::values::value::Len, side: f32, auto: f32| match l {
        crate::style::values::value::Len::Px(v) => v,
        crate::style::values::value::Len::Pct(p) => p * side,
        _ => auto,
    };
    // Несколько слоёв или снимок определения из документа:
    // полотно, собранное по `mask-composite` (css-masking §7.12).
    // Плитка слоя — его интринзик (auto), укладка от угла коробки;
    // уложенное полотно уходит одной плиткой без мощения.
    let referenced = layers.iter().any(|l| {
        l.starts_with("svgsnap:")
            || l.starts_with("clipsnap:")
            || l.starts_with("pathdef:")
            || l.starts_with("shapedef:")
            || (l.contains('#') && l.contains(".svg"))
    });
    if layers.len() > 1 || referenced || !group.mask_repeat_modes.is_empty() {
        return composite_mask(group, &layers, bounds, bw, bh, ol, ot, window, len);
    }
    let source = crate::paint::background::source(src)?;
    let (img, tw, th) = match &source {
        crate::paint::background::Source::Raster(img) => {
            let (tw, th) = mask_size::tile(
                source.intrinsic(),
                group.mask_scale,
                (bw, bh),
                group.mask_size,
                group.mask_fit,
            );
            (img.clone(), tw, th)
        }
        _ => {
            let (tw, th) = mask_size::tile(
                source.intrinsic(),
                group.mask_scale,
                (bw, bh),
                group.mask_size,
                group.mask_fit,
            );
            // Растр — в физических точках окна: маска в CSS-точках
            // растягивалась при композите и мылила край формы
            // (clip-path-circle-010 и родня: 0.71 вместо нуля).
            let sf = window.scale_factor();
            let img = match &source {
                crate::paint::background::Source::Shape { raw }
                    if !raw.trim_start().starts_with("rrect(") =>
                {
                    // Форма может выйти за коробку — растр кроет
                    // расширенную область, центр смещён на вынос.
                    //
                    // Радиусы и центр считаются от ОПОРНОЙ КОРОБКИ
                    // формы (css-masking-1 §1.3.1.1): её края несёт
                    // `poly_expand`, как и у полигона. Прежде круг
                    // всегда мерился border-box, и
                    // `circle(farthest-side) content-box` выходил
                    // радиусом во всю коробку
                    // (`clip-path-contentBox-1a/1d/1e`,
                    // `-fillBox-*`, `-viewBox-*`).
                    let [pt, pr, pb, pl] = group.poly_expand;
                    let (rw, rh) = ((bw + pl + pr).max(1.0), (bh + pt + pb).max(1.0));
                    let (cx0, cy0, rx, ry) =
                        crate::paint::background::shape_params(raw, rw, rh, 1.0)?;
                    let (cx, cy) = (cx0 - pl, cy0 - pt);
                    let (aw, ah) = (bw + sl + sr, bh + st + sb);
                    let img = crate::paint::background::rasterize_ellipse_px(
                        (cx + sl) * sf,
                        (cy + st) * sf,
                        rx * sf,
                        ry * sf,
                        (aw * sf).round().max(1.0) as u32,
                        (ah * sf).round().max(1.0) as u32,
                    )?;
                    return Some((
                        img,
                        Bounds {
                            origin: gpui::point(bounds.origin.x - px(sl), bounds.origin.y - px(st)),
                            size: gpui::size(px(aw), px(ah)),
                        },
                        // Одна плитка: за пределами области пусто.
                        3,
                    ));
                }
                crate::paint::background::Source::Shape { raw } => {
                    // Точечные величины формы записаны в CSS-точках —
                    // растеризатору нужен их масштаб.
                    crate::paint::background::rasterize_shape(
                        raw,
                        (tw * sf).round().max(1.0) as u32,
                        (th * sf).round().max(1.0) as u32,
                        sf,
                    )?
                }
                // Рисунок со СВОИМ размером растрируется в нём:
                // без viewBox при большем вьюпорте содержимое НЕ
                // растёт, и плитка выходила с прозрачными полосами
                // (mask-repeat-1: щели; mask-size-cover: четверть).
                // На плитку его натянет сэмплер композита. Без
                // своего размера — точно в плитку (CSS-точки,
                // чёткость даёт плотность растеризатора).
                // Плитка в CSS-точках; рисунок масштабирует
                // with_viewport (viewBox из своих размеров).
                _ => source.mask_raster((tw, th), sf)?,
            };
            (img, tw, th)
        }
    };
    let (ox, oy) = match group.mask_pos {
        Some((x, y)) => {
            // Доля — от свободного места; `right/bottom` зеркалит
            // отсчёт (css-backgrounds §3.6, mask-position-1a).
            let one = |l: crate::style::values::value::Len, free: f32, far: bool| {
                let v = match l {
                    crate::style::values::value::Len::Pct(p) => p * free,
                    l => len(l, free, 0.0),
                };
                if far { free - v } else { v }
            };
            (
                one(x, bw - tw, group.mask_pos_far.0),
                one(y, bh - th, group.mask_pos_far.1),
            )
        }
        None => (0.0, 0.0),
    };
    Some((
        img,
        mask_size::snap_tile(
            gpui::point(bounds.origin.x + px(ol + ox), bounds.origin.y + px(ot + oy)),
            (tw, th),
            window.scale_factor(),
        ),
        ((group.mask_no_repeat.0 as u32)
            | ((group.mask_no_repeat.1 as u32) << 1)
            | ((group.mask_luminance as u32) << 2)),
    ))
}
