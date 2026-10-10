//! Сгруппированная покраска (непрозрачность, обрезка).
mod bare_clip;
pub(super) use bare_clip::bare_clip;

mod wrapper;
pub(crate) use wrapper::grouped_wrapper;

// owner: A

use crate::paint::effects::mask::{PAINT_VIEWPORT, resolve_mask_refs};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::AnyElement;

/// Точки стороны коробки: только явный `px` (None непроходной).
pub(crate) fn px_of2(l: &Option<Len>) -> Option<f32> {
    match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    }
}

/// `inset()`/`rect()`/`xywh()` with a `round` radius in points: the group
/// buffer rounds the clip rectangle (css-shapes-1 §basic-shape-rect);
/// percentages resolve against the reference box at paint time.
pub(crate) fn rounded_rect_clip(c: &Computed) -> bool {
    matches!(c.clip_round_len, Some(Len::Px(v) | Len::Pct(v)) if v > 0.0)
        && (c.clip_inset.is_some()
            || c.clip_edges.is_some()
            || c.clip_xywh.is_some()
            || c.clip_polygon.is_some())
}

/// Отрисовать поддерево в отдельный буфер, когда эффекту нужна готовая
/// картинка целиком.
///
/// Таких случаев три: размытие поддерева (`filter: blur`), смешивание с
/// кадром по формулам CSS (`mix-blend-mode`) и изоляция (`isolation`), где
/// поддерево обязано сложиться отдельно, прежде чем попасть в кадр.
pub(crate) fn grouped(el: AnyElement, c: &Computed) -> AnyElement {
    let blur = c.filter.map_or(0.0, |f| f.blur);
    let blend = c.blend.unwrap_or(0);
    // Вершины полигона в `em`/`ex`/`ch`/`vw`/`vh` (css-shapes-1 `polygon()`:
    // `<length-percentage>`) меряются ЗДЕСЬ, как у `clip: rect()` ниже: на
    // разборе кегль и окно неизвестны, а отрисовка (`Grouped::paint`, `coord`)
    // знает только точки и доли — остальное шло нулём, и полоса схлопывалась
    // в линию (clip-path-polygon-013: 4 полосы из 6, 30400/480000 = 6.33).
    // Смесь `calc()` по-прежнему отбрасывается ещё на разборе.
    let poly_font = match c.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let poly_family = c.font_family.clone().unwrap_or_default();
    let poly_vp = PAINT_VIEWPORT.with(|v| v.get());
    let poly_unit = |l: Len| -> Len {
        match l {
            Len::Px(_) | Len::Pct(_) => l,
            Len::Vw(k) => Len::Px(k * poly_vp.0),
            Len::Vh(k) => Len::Px(k * poly_vp.1),
            other => crate::text::metrics::fallback_len_px(other, &poly_family, poly_font)
                .map(Len::Px)
                .unwrap_or(other),
        }
    };
    let polygon_px: Vec<(Len, Len)> = c
        .clip_polygon
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|&(x, y)| (poly_unit(x), poly_unit(y)))
        .collect();
    let polygon = polygon_px.as_slice();
    // Маска-изображение (css-masking §7.1): источник уходит строкой, его
    // альфа гасит готовый буфер группы при композите; резолв — при
    // отрисовке, когда известен размер коробки. Базовая форма `clip-path`
    // (circle/ellipse) идёт тем же путём — растровой альфа-маской, как и
    // эллиптический `border-radius: H / V` (углы rx≠ry растеризатор круглить
    // не умеет; круглые пары дополняются из обычного радиуса).
    // Большой НЕОДНОРОДНЫЙ круглый радиус — тоже маской: растеризатор жмёт
    // каждый угол к половине меньшей стороны, а спека — одним множителем от
    // суммы СМЕЖНЫХ радиусов (§5.5): `border-radius: 100px 100px 0 0` на
    // 200x100 — законный полукруг, растеризатор рисовал стадион
    // (clip-path-semicircle-ref). Однородные радиусы совпадают с растеризатором.
    // Фигурные углы (`corner-shape`, css-borders-4) — той же маской: запись
    // несёт радиусы (точки либо доли, резолв при растре) и параметр K по углам.
    // Радиус в единицах шрифта на `e.style` ещё не разрешён: `rrect_spec`
    // читает только точки и доли и писал бы `0` — маску БЕЗ скругления поверх
    // квада, с которого `apply_radius` скругление снял. Меряем тем же
    // `poly_unit`, что вершины полигона (`contain-paint-clip-002`: 4em = 64 →
    // ужатие css-backgrounds-3 §5.5 до 60). Blink берёт форму обрезки из
    // вычисленного стиля (`paint_property_tree_builder.cc:3127-3143`).
    let rrect = c.radius_masked().then(|| {
        let mut own = c.clone();
        own.resolve_radius_lengths(|r| *r = r.map(poly_unit));
        format!("shape:{}", crate::paint::background::rrect_spec(&own, None))
    });
    // `border-shape` (css-borders-4): фон и содержимое режутся ВНЕШНИМ
    // контуром рамки (Blink клипует фон внешней фигурой, у двух фигур —
    // внутренней; кольцо у нас лежит непрозрачным слоем сверху, итог тот же).
    // Две фигуры при ПРОЗРАЧНОЙ рамке: кольца не видно, а фон обязан
    // исчезнуть вместе с внутренней фигурой — маска берёт её
    // (border-shape-collapsed-shape-clips-background, t3). Запись:
    // `t r b l` опорной коробки, обводка, `t r b l` выноса, `:`, фигура.
    let bshape = c.border_shape.as_ref().map(|bs| {
        let (stroke, colour) = c.border_shape_stroke();
        // Рамка, несущая заливку `border-area`, видима — маска берёт внешнюю
        // фигуру, как у цветной рамки.
        let colour = crate::paint::background::border_paint(c, colour);
        // Обрезка переполнения — ВНУТРЕННИМ контуром (css-borders-4
        // §border-shape-overflow-interaction; Blink `InnerPath`): у двух фигур
        // внутренняя, у одной — внешняя минус обводка (отрицательная обводка
        // в записи, `background::border_shape_mask_svg`). Кольцо при этом
        // ложится НАД буфером (`Grouped::over`, ниже).
        let (shape, kind, stroke) = match &bs.inner {
            Some((inner, k)) if colour.a <= 0.0 || c.border_shape_clips() => {
                (inner.as_str(), *k, 0.0)
            }
            Some(_) => (bs.outer.as_str(), bs.outer_box, 0.0),
            None if c.border_shape_clips() => (bs.outer.as_str(), bs.outer_box, -stroke),
            None => (bs.outer.as_str(), bs.outer_box, stroke),
        };
        let [ot, or_, ob, ol] = c.geometry_outsets(kind);
        let [et, er, eb, el] = c.border_shape_ext();
        format!("bordershape:{ot} {or_} {ob} {ol} {stroke} {et} {er} {eb} {el}:{shape}")
    });
    // Шрифтовые единицы в командах `shape()` — СВОИМ кеглем и семейством
    // (css-values-4 §6.1): `shape_to_path` на отрисовке знает только запасной
    // кегль 16 и системный шрифт, и `2ch`/`10em` при `font: 5px Ahem`
    // выходили ≈17.8/160 вместо 10/50 (clip-path-shape-002-units 0.81).
    // Корневой кегль — тот же, что читает `Len::parse` для `rem`.
    let clip_shape = c.clip_shape.clone().map(|s| {
        if !s.starts_with("shapedef:") {
            return s;
        }
        let px = match c.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let family = c.font_family.clone().unwrap_or_default();
        let (ch, ex) = crate::text::metrics::ch_ex_px(&family, px);
        crate::style::computed::font_lengths_to_px(
            &s,
            px,
            crate::style::values::value::root_font_px(),
            ex,
            ch,
        )
    });
    let mask = c
        .mask_image
        .clone()
        .or(clip_shape)
        .or(bshape)
        .or(rrect)
        .map(|m| resolve_mask_refs(&m));
    // `clip: rect()` действует только на абсолютный элемент (CSS 2.1).
    // ПАРК: буфер группы создаёт stacking context, которого у `clip` нет —
    // z-переплетение детей с внешними соседями рвётся
    // (clip-no-stacking-context, 1 пара).
    // Единицы шрифта в `clip: rect(...)` меряются ЗДЕСЬ: на разборе кегль ещё
    // неизвестен, а к отрисовке он уже слит. Прежде ненулевые `em`/`ex`
    // читались как `auto`, и обрезки не было вовсе (`visufx/clip-079` и родня).
    let clip_rect = c
        .clip_len
        .map(|sides| {
            let base = match c.font_size {
                Some(Len::Px(v)) => v,
                _ => 16.0,
            };
            let family = c.font_family.clone().unwrap_or_default();
            sides.map(|l| match l {
                Some(Len::Px(v)) => Some(v),
                Some(other) => Some(crate::text::metrics::spacing_px(Some(other), &family, base)),
                None => None,
            })
        })
        .or(c.clip_rect)
        .filter(|_| {
            matches!(
                c.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            )
        });
    // Голое слово коробки — срез краями этой коробки от border-box:
    // margin-box шире на поля, padding-box уже на рамку, content-box — на
    // рамку и отбивку (clip-path-marginBox-*, -paddingBox-*, -contentBox-*).
    let (bare_inset, bare_round) = bare_clip(c);
    let clip_inset = c.clip_inset.or(bare_inset);
    if blur <= 0.0
        && blend == 0
        && polygon.is_empty()
        && c.isolate != Some(true)
        && mask.is_none()
        && clip_rect.is_none()
        && clip_inset.is_none()
        && c.clip_edges.is_none()
        && c.clip_xywh.is_none()
    {
        return el;
    }
    // Чистая изоляция — буфер без собственного эффекта: ни размытия, ни
    // смешивания, ни маски, ни обрезки. Такой буфер коробкой не режется
    // (`interact::Grouped::spill`).
    grouped_wrapper(
        blur, blend, polygon, mask, clip_rect, clip_inset, c, el, bare_round,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_mask_composite(
    wrapper: &mut crate::paint::effects::grouped_element::Grouped,
    c: &Computed,
) {
    wrapper.mask_composite = c.mask_composite.clone().unwrap_or_default();
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_mask_style(
    wrapper: &mut crate::paint::effects::grouped_element::Grouped,
    c: &Computed,
) {
    wrapper.mask_size = c.mask_size;
    wrapper.mask_fit = c.mask_fit.unwrap_or(0);
    wrapper.mask_no_repeat = c.mask_no_repeat.unwrap_or((false, false));
    wrapper.mask_repeat_list = c.mask_repeat_list.clone().unwrap_or_default();
    wrapper.mask_repeat_modes = c.mask_repeat_modes.clone().unwrap_or_default();
    wrapper.mask_luminance = c.mask_luminance == Some(true);
    wrapper.mask_alpha_mode = c.mask_alpha_mode == Some(true);
    wrapper.mask_pos = c.mask_pos;
    wrapper.mask_pos_far = c.mask_pos_far;
    wrapper.mask_pos_list = c.mask_pos_list.clone().unwrap_or_default();
}
