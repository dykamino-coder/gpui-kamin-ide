//! Растр и укладка отдельного слоя составной маски.

use crate::paint::effects::grouped_element::{Grouped, rasterize_mask_def, svg_fragment};
use crate::paint::effects::mask_size;

#[allow(clippy::too_many_arguments)]
pub(crate) fn mask_layer(
    i: usize,
    l: &str,
    group: &Grouped,
    bw: f32,
    bh: f32,
    sf: f32,
    pos_of: &impl Fn(usize, f32, f32) -> (f32, f32),
    repeat_of: &impl Fn(usize) -> (bool, bool),
) -> Option<crate::paint::background::MaskLayer> {
    let mut space_gap = (0.0, 0.0);
    let mut space_once = (false, false);
    // Ссылка на определение в документе: растр под
    // коробку, светимость вместо альфы.
    let (image, tile, lum) = if let Some(id) = l.strip_prefix("svgsnap:") {
        // css-masking-1 §7.2: `match-source` у ссылки на
        // `<mask>` — его `mask-type` (светимость по
        // умолчанию); явный `mask-mode` главнее.
        let alpha = !group.mask_luminance && (group.mask_alpha_mode || id.ends_with('A'));
        (
            rasterize_mask_def(id, bw, bh, false)?,
            [0.0, 0.0, bw * sf, bh * sf],
            !alpha,
        )
    } else if let Some(id) = l.strip_prefix("clipsnap:") {
        (
            rasterize_mask_def(id, bw, bh, true)?,
            [0.0, 0.0, bw * sf, bh * sf],
            true,
        )
    } else if let Some(markup) = l
        .rsplit_once('#')
        .filter(|(f, _)| f.ends_with(".svg"))
        .and_then(|(file, frag)| svg_fragment(file, frag))
    {
        // CSS Masking §7.1: a <mask> reference is distinct
        // from an SVG image URL with an ordinary fragment.
        let markup = markup.replace("clip-rule", "fill-rule");
        let markup = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{bw}" height="{bh}">{markup}</svg>"#
        );
        (
            crate::svg::raster::rasterize(&markup, bw, bh)?,
            [0.0, 0.0, bw * sf, bh * sf],
            true,
        )
    } else if let Some(rest) = l.strip_prefix("shapedef:") {
        // Команды `shape()` переводятся в контур `d`
        // с резолвом долей по ОПОРНОЙ коробке формы
        // (`shape(...) content-box`, css-masking
        // §1.3.1.1): её края несёт poly_expand, контур
        // сдвигается на них внутрь.
        let (rule, body) = rest.split_once(':')?;
        let [et, _er, _eb, el] = group.poly_expand;
        let (fw2, fh2) = (
            (bw + el + group.poly_expand[1]).max(1.0),
            (bh + et + group.poly_expand[2]).max(1.0),
        );
        let d = crate::paint::background::shape_to_path(body, fw2, fh2)?;
        let (dx, dy) = (-el, -et);
        let markup = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{bw}" height="{bh}"><g transform="translate({dx} {dy})"><path fill="#ffffff" fill-rule="{rule}" d="{d}"/></g></svg>"##
        );
        (
            crate::svg::raster::rasterize(&markup, bw, bh)?,
            [0.0, 0.0, bw * sf, bh * sf],
            true,
        )
    } else if let Some(rest) = l.strip_prefix("pathdef:") {
        // Контур `path()`: белая заливка с правилом
        // намотки, светимость = покрытие.
        let (rule, d) = rest.split_once(':')?;
        let markup = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{bw}" height="{bh}"><path fill="#ffffff" fill-rule="{rule}" d="{d}"/></svg>"##
        );
        (
            crate::svg::raster::rasterize(&markup, bw, bh)?,
            [0.0, 0.0, bw * sf, bh * sf],
            true,
        )
    } else {
        let source = crate::paint::background::source(l)?;
        let (tw, th) = mask_size::tile(
            source.intrinsic(),
            group.mask_scale,
            (bw, bh),
            group.mask_size,
            group.mask_fit,
        );
        let modes = group.mask_repeat_modes.as_slice();
        let (mx, my) = if modes.is_empty() {
            (0, 0)
        } else {
            modes[i % modes.len()]
        };
        let (tw, th) =
            mask_size::round_tile((tw, th), (bw, bh), (mx == 3, my == 3), group.mask_size);
        // Плитка кладётся не в угол, а в точку СВОЕГО
        // слоя: без этого `mask-position: top, bottom`
        // сваливал оба слоя в (0,0) (mask-position-5).
        let (ox, oy) = pos_of(i, tw, th);
        // `space` (css-backgrounds-3 §3.4): whole tiles
        // with equal gaps, the outer ones touching the
        // edges; position only acts when fewer than two fit.
        let (ox, gx, nx) = mask_size::space_axis(mx == 2, ox, tw, bw);
        let (oy, gy, ny) = mask_size::space_axis(my == 2, oy, th, bh);
        space_gap = (gx * sf, gy * sf);
        space_once = (nx, ny);
        (
            source.mask_raster((tw, th), sf)?,
            [ox * sf, oy * sf, tw * sf, th * sf],
            false,
        )
    };
    let no_repeat = repeat_of(i);
    Some(crate::paint::background::MaskLayer {
        image,
        tile,
        no_repeat: (no_repeat.0 || space_once.0, no_repeat.1 || space_once.1),
        gap: space_gap,
        snap: !group.mask_repeat_modes.is_empty(),
        // Список операторов КОРОЧЕ набора слоёв
        // повторяется (css-masking-1 §7.12 ->
        // css-backgrounds-3 §2.2): прежде слоям сверх
        // длины доставался `add`, и `mask-composite:
        // subtract` из трёх слоёв считался только на
        // верхнем (mask-composite-1d 3.17). При ДВУХ
        // слоях итог не меняется: оператор нижнего слоя
        // не читается вовсе (ветка `first` в
        // `compose_mask_layers`).
        op: if group.mask_composite.is_empty() {
            0
        } else {
            group.mask_composite[i % group.mask_composite.len()]
        },
        luminance: lum || group.mask_luminance,
    })
}
