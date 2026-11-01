//! Настройка состояния Grouped после вычисления геометрии клипа.

use super::rounded_rect_clip;
use crate::hooks::IntoHooked;
use crate::layout::cx::FrameCx;
use crate::paint::effects::mask::mask_geometry;
use crate::style::computed::BoxStyle;
use crate::style::values::value::{Len, px_lossy_zero};
use gpui::AnyElement;

#[allow(clippy::too_many_arguments)]
pub(crate) fn grouped_wrapper(
    blur: f32,
    blend: u8,
    polygon: &[(Len, Len)],
    mask: Option<crate::paint::effects::grouped_element::GroupMask>,
    clip_rect: Option<[Option<f32>; 4]>,
    clip_inset: Option<[Len; 4]>,
    c: &BoxStyle,
    el: AnyElement,
    bare_round: Option<Len>,
    ring_over: Option<(String, crate::style::values::value::Color, [f32; 4])>,
    frame: FrameCx<'_>,
) -> AnyElement {
    let vcx = frame.values;
    let values = vcx.keep();
    let pure_isolation = blur <= 0.0
        && blend == 0
        && polygon.is_empty()
        && mask.is_none()
        && clip_rect.is_none()
        && clip_inset.is_none()
        && c.reset_mask.clip_edges.is_none()
        && c.reset_mask.clip_xywh.is_none();
    let mut wrapper = crate::paint::effects::grouped_element::Grouped::new(el, frame);
    wrapper.spill = pure_isolation || mask_geometry::unclipped(c);
    wrapper.blur = blur;
    wrapper.blend = u32::from(blend);
    wrapper.mask = mask;
    // Наружные тени коробки с `border-shape` повторяют фигуру и лежат
    // СНАРУЖИ неё — под буфером группы и вне его маски (css-borders-4
    // §border-shape-shadow-interaction; Blink `PaintNormalBoxShadow`, ветка
    // `HasBorderShape`). Квад тени (`apply::apply_paint`) и слой резкой
    // тени (`decorations`) у такой коробки не ставятся.
    if let Some(bs) = c.reset_border.border_shape.clone()
        && !c.paint_shadows().is_empty()
    {
        let (stroke, _) = c.border_shape_stroke();
        let outer_out = c.geometry_outsets(bs.outer_box);
        let inner = bs.inner.clone().map(|(s, k)| (s, c.geometry_outsets(k)));
        let shadows = c.resolved_shadows(false);
        let values = values.clone();
        wrapper.under = Some(Box::new(move |bw, bh, sl, st, aw, ah| {
            let frame = values.vcx();
            crate::paint::background::border_shape_shadow_scene(
                (bs.outer.as_str(), outer_out),
                inner.as_ref().map(|(s, o)| (s.as_str(), *o)),
                stroke,
                &shadows,
                false,
                bw,
                bh,
                sl,
                st,
                aw,
                ah,
                frame,
            )
        }));
    }
    // Кольцо рамки `border-shape` при обрезке переполнения — НАД буфером
    // группы: содержимое и фон режутся внутренним контуром (маска выше), а
    // рамка лежит снаружи него и поверх обрезанных детей, как в Blink
    // (`PaintBorderShape` после детей не нужен — дети до внутреннего контура
    // не доходят). В декорациях кольцо тогда не ставится.
    if let Some(bs) = c.reset_border.border_shape.clone()
        && c.border_shape_clips()
    {
        let (stroke, colour) = c.border_shape_stroke();
        let colour = crate::paint::background::border_paint(c, colour, vcx);
        let outer_out = c.geometry_outsets(bs.outer_box);
        let inner = bs.inner.clone().map(|(s, k)| (s, c.geometry_outsets(k)));
        if (inner.is_some() || stroke > 0.0) && colour.a > 0.0 {
            let values = values.clone();
            wrapper.over.push(Box::new(move |bw, bh, sl, st, aw, ah| {
                let frame = values.vcx();
                crate::paint::background::border_shape_ring_scene(
                    (bs.outer.as_str(), outer_out),
                    inner.as_ref().map(|(s, o)| (s.as_str(), *o)),
                    stroke,
                    colour,
                    bw,
                    bh,
                    sl,
                    st,
                    aw,
                    ah,
                    frame,
                )
            }));
        }
    }
    // Контур `outline` повторяет фигуру (Blink `BorderShapePainter::
    // PaintOutline`): полоса по внешнему контуру, над группой — контур лежит
    // снаружи фигуры и красится последним (CSS 2.1 прил. E, шаг 10), а маска
    // группы его бы срезала. Квад контура в декорациях не ставится.
    if let Some(bs) = c.reset_border.border_shape.clone()
        && let Some((w, off, colour)) = c.shaped_outline()
        && colour.a > 0.0
    {
        let (stroke, _) = c.border_shape_stroke();
        let outer_out = c.geometry_outsets(bs.outer_box);
        let single = bs.inner.is_none();
        let double = c
            .reset_border
            .outline
            .as_ref()
            .is_some_and(|o| o.style == Some(crate::style::computed::OUTLINE_DOUBLE));
        wrapper.over.push(Box::new(move |bw, bh, sl, st, aw, ah| {
            let frame = values.vcx();
            crate::paint::background::border_shape_outline_scene(
                (bs.outer.as_str(), outer_out),
                single,
                stroke,
                off,
                w,
                double,
                colour,
                bw,
                bh,
                sl,
                st,
                aw,
                ah,
                frame,
            )
        }));
    }
    // Резкие тени и контур `corner-shape` — по раздвинутому контуру
    // (css-borders-4 §corner-shaping), подложкой и накладкой группы.
    {
        use crate::paint::decorations::shaped_outset as so;
        if let Some(layer) = so::shadow_layer(c, vcx) {
            wrapper.under = Some(layer);
        }
        wrapper.under_fills = so::shadow_fills(c, vcx);
        wrapper.over_fills = so::outline_layer(c, vcx);
        wrapper.deco_ext = so::reach(c, vcx);
    }
    super::apply_mask_style(&mut wrapper, c);
    // Коробки маски (css-masking §7.10-7.11): сдвиги краёв от border-box
    // внутрь — рамка (padding-box) либо рамка+отступ (content-box).
    let box_off = |kind| mask_geometry::offsets(c, kind);
    wrapper.mask_origin_off = box_off(c.reset_mask.mask_origin);
    wrapper.clip_rect = clip_rect;
    wrapper.clip_inset = clip_inset;
    wrapper.clip_edges = c.reset_mask.clip_edges;
    wrapper.clip_xywh = c.reset_mask.clip_xywh;
    if rounded_rect_clip(c) {
        wrapper.clip_round = c.reset_mask.clip_round_len;
    } else if bare_round.is_some() {
        wrapper.clip_round = bare_round;
    }
    // `clip`/`mask-clip` живут в системе координат элемента ДО трансформа, а
    // трансформ рисуется ВНУТРИ буфера группы — коробка клипа обязана ехать
    // вместе (clip-transform-order: сдвинутый рисунок резался по старому
    // месту). Честно поддержан только сдвиг; поворот с клипом — парк.
    if let Some(t) = &c.reset_transforms.transform {
        wrapper.clip_shift = (
            t.translate.0,
            t.translate.1,
            t.translate_pct.0,
            t.translate_pct.1,
        );
    }
    super::apply_mask_composite(&mut wrapper, c);
    wrapper.mask_clip_off = c
        .reset_mask
        .mask_clip
        .filter(|k| *k != 255)
        .map(|k| box_off(Some(k)));
    // SVG-ребёнок: коробки маски уже посчитаны от его stroke-box
    // (`svg::masked_layers`), рамки и отбивки у него нет.
    if let Some((origin, clip)) = c.reset_mask.mask_box_override {
        wrapper.mask_origin_off = origin;
        wrapper.mask_clip_off = clip;
    }
    if c.reset_mask.mask_user_scale > 0.0 {
        wrapper.mask_scale = c.reset_mask.mask_user_scale;
    }
    // Точки уходят КАК ЕСТЬ (Len): проценты и пиксели резолвятся при
    // отрисовке от опорной коробки формы (css-masking §1.3.1.1): margin-box
    // расширяет bounds на поля, content-box сужает на рамку+паддинг
    // (clip-path-polygon-008: полигон в margin-box; masking 82→84).
    wrapper.polygon = polygon.to_vec();
    wrapper.polygon_evenodd = c.reset_mask.clip_polygon_evenodd;
    let side = px_lossy_zero;
    let b = c.borders();
    wrapper.poly_expand = match c.reset_mask.clip_ref {
        Some(1) => {
            let m = c
                .reset_box_layout
                .own_margin
                .unwrap_or(c.reset_box_layout.margin);
            [side(m.top), side(m.right), side(m.bottom), side(m.left)]
        }
        Some(2) => [-side(b.top), -side(b.right), -side(b.bottom), -side(b.left)],
        Some(3) => [
            -(side(b.top) + side(c.reset_box_layout.padding.top)),
            -(side(b.right) + side(c.reset_box_layout.padding.right)),
            -(side(b.bottom) + side(c.reset_box_layout.padding.bottom)),
            -(side(b.left) + side(c.reset_box_layout.padding.left)),
        ],
        _ => [0.0; 4],
    };
    wrapper.ring_over = ring_over;
    wrapper.corner_clip = crate::paint::effects::corner_clip::new(
        c,
        wrapper.mask.as_ref().and_then(|m| m.geometry()),
        frame,
    );
    if wrapper.corner_clip.is_some() {
        wrapper.opacity = c.reset_effects.opacity.unwrap_or(1.0);
    }
    wrapper.hooked()
}
