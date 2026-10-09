//! Сгруппированная покраска (непрозрачность, обрезка).
// owner: A

use crate::render::*;

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
            other => crate::metrics::fallback_len_px(other, &poly_family, poly_font)
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
        format!("shape:{}", crate::background::rrect_spec(&own, None))
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
        let colour = crate::background::border_paint(c, colour);
        // Обрезка переполнения — ВНУТРЕННИМ контуром (css-borders-4
        // §border-shape-overflow-interaction; Blink `InnerPath`): у двух фигур
        // внутренняя, у одной — внешняя минус обводка (отрицательная обводка
        // в записи, `background::border_shape_mask_svg`). Кольцо при этом
        // ложится НАД буфером (`Grouped::over`, ниже).
        let (shape, kind, stroke) = match &bs.inner {
            Some((inner, k)) if colour.a <= 0.0 || c.border_shape_clips() => (inner.as_str(), *k, 0.0),
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
        let (ch, ex) = crate::metrics::ch_ex_px(&family, px);
        crate::computed::font_lengths_to_px(&s, px, crate::value::root_font_px(), ex, ch)
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
                Some(other) => Some(crate::metrics::spacing_px(Some(other), &family, base)),
                None => None,
            })
        })
        .or(c.clip_rect)
        .filter(|_| {
        matches!(
            c.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        )
    });
    // Голое слово коробки — срез краями этой коробки от border-box:
    // margin-box шире на поля, padding-box уже на рамку, content-box — на
    // рамку и отбивку (clip-path-marginBox-*, -paddingBox-*, -contentBox-*).
    let bare_inset = if c.clip_bare_box && c.clip_inset.is_none() {
        let b = c.borders();
        let s = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let [t, r, bo, l] = match c.clip_ref {
            Some(1) => [
                -s(c.margin.top),
                -s(c.margin.right),
                -s(c.margin.bottom),
                -s(c.margin.left),
            ],
            Some(2) => [s(b.top), s(b.right), s(b.bottom), s(b.left)],
            Some(3) => [
                s(b.top) + s(c.padding.top),
                s(b.right) + s(c.padding.right),
                s(b.bottom) + s(c.padding.bottom),
                s(b.left) + s(c.padding.left),
            ],
            _ => [0.0; 4],
        };
        Some([Len::Px(t), Len::Px(r), Len::Px(bo), Len::Px(l)])
    } else {
        None
    };
    // Голая коробка режет ВМЕСТЕ со скруглением углов (css-masking-1
    // §5.1 «<geometry-box>… including any corner shaping (e.g.
    // border-radius)»): радиус коробки — радиус рамки, сдвинутый на ту же
    // толщину (css-backgrounds-3 §5.2 inner/outer curves). Пока — только
    // равные круглые углы и равные стороны сдвига.
    let bare_round = bare_inset.and_then(|inset| {
        if c.radius_ell.is_some() {
            return None;
        }
        let r = match (c.radius.tl, c.radius.tr, c.radius.br, c.radius.bl) {
            (Some(Len::Px(a)), Some(Len::Px(b)), Some(Len::Px(d)), Some(Len::Px(e)))
                if a == b && b == d && d == e && a > 0.0 =>
            {
                a
            }
            _ => return None,
        };
        let d = match inset {
            [Len::Px(t), Len::Px(rr), Len::Px(bo), Len::Px(l)]
                if t == rr && rr == bo && bo == l =>
            {
                t
            }
            _ => return None,
        };
        // margin-box наружу: радиус растёт на поле, когда он не меньше поля
        // (css-shapes-1 §6.1 — при r < m нужна поправка, её здесь нет).
        if d < 0.0 && r < -d {
            return None;
        }
        let r = (r - d).max(0.0);
        (r > 0.0).then_some(Len::Px(r))
    });
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
    let pure_isolation = blur <= 0.0
        && blend == 0
        && polygon.is_empty()
        && mask.is_none()
        && clip_rect.is_none()
        && clip_inset.is_none()
        && c.clip_edges.is_none()
        && c.clip_xywh.is_none();
    let mut wrapper = crate::interact::Grouped::new(el);
    wrapper.spill = pure_isolation || mask_geometry::unclipped(c);
    wrapper.blur = blur;
    wrapper.blend = u32::from(blend);
    wrapper.mask = mask;
    // Наружные тени коробки с `border-shape` повторяют фигуру и лежат
    // СНАРУЖИ неё — под буфером группы и вне его маски (css-borders-4
    // §border-shape-shadow-interaction; Blink `PaintNormalBoxShadow`, ветка
    // `HasBorderShape`). Квад тени (`apply::apply_paint`) и слой резкой
    // тени (`decorations`) у такой коробки не ставятся.
    if let Some(bs) = c.border_shape.clone()
        && !c.shadows.is_empty()
    {
        let (stroke, _) = c.border_shape_stroke();
        let outer_out = c.geometry_outsets(bs.outer_box);
        let inner = bs
            .inner
            .clone()
            .map(|(s, k)| (s, c.geometry_outsets(k)));
        let shadows = c.resolved_shadows(false);
        wrapper.under = Some(Box::new(move |bw, bh, sl, st, aw, ah| {
            crate::background::border_shape_shadow_svg(
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
            )
        }));
    }
    // Кольцо рамки `border-shape` при обрезке переполнения — НАД буфером
    // группы: содержимое и фон режутся внутренним контуром (маска выше), а
    // рамка лежит снаружи него и поверх обрезанных детей, как в Blink
    // (`PaintBorderShape` после детей не нужен — дети до внутреннего контура
    // не доходят). В декорациях кольцо тогда не ставится.
    if let Some(bs) = c.border_shape.clone()
        && c.border_shape_clips()
    {
        let (stroke, colour) = c.border_shape_stroke();
        let colour = crate::background::border_paint(c, colour);
        let outer_out = c.geometry_outsets(bs.outer_box);
        let inner = bs
            .inner
            .clone()
            .map(|(s, k)| (s, c.geometry_outsets(k)));
        if (inner.is_some() || stroke > 0.0) && colour.a > 0.0 {
            wrapper.over.push(Box::new(move |bw, bh, sl, st, aw, ah| {
                crate::background::border_shape_ring_svg(
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
                )
            }));
        }
    }
    // Контур `outline` повторяет фигуру (Blink `BorderShapePainter::
    // PaintOutline`): полоса по внешнему контуру, над группой — контур лежит
    // снаружи фигуры и красится последним (CSS 2.1 прил. E, шаг 10), а маска
    // группы его бы срезала. Квад контура в декорациях не ставится.
    if let Some(bs) = c.border_shape.clone()
        && let Some((w, off, colour)) = c.shaped_outline()
        && colour.a > 0.0
    {
        let (stroke, _) = c.border_shape_stroke();
        let outer_out = c.geometry_outsets(bs.outer_box);
        let single = bs.inner.is_none();
        let double = c
            .outline
            .as_ref()
            .is_some_and(|o| o.style == Some(crate::computed::OUTLINE_DOUBLE));
        wrapper.over.push(Box::new(move |bw, bh, sl, st, aw, ah| {
            crate::background::border_shape_outline_svg(
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
            )
        }));
    }
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
    // Коробки маски (css-masking §7.10-7.11): сдвиги краёв от border-box
    // внутрь — рамка (padding-box) либо рамка+отступ (content-box).
    let box_off = |kind| mask_geometry::offsets(c, kind);
    wrapper.mask_origin_off = box_off(c.mask_origin);
    wrapper.clip_rect = clip_rect;
    wrapper.clip_inset = clip_inset;
    wrapper.clip_edges = c.clip_edges;
    wrapper.clip_xywh = c.clip_xywh;
    if rounded_rect_clip(c) {
        wrapper.clip_round = c.clip_round_len;
    } else if bare_round.is_some() {
        wrapper.clip_round = bare_round;
    }
    // `clip`/`mask-clip` живут в системе координат элемента ДО трансформа, а
    // трансформ рисуется ВНУТРИ буфера группы — коробка клипа обязана ехать
    // вместе (clip-transform-order: сдвинутый рисунок резался по старому
    // месту). Честно поддержан только сдвиг; поворот с клипом — парк.
    if let Some(t) = &c.transform {
        wrapper.clip_shift = (
            t.translate.0,
            t.translate.1,
            t.translate_pct.0,
            t.translate_pct.1,
        );
    }
    wrapper.mask_composite = c.mask_composite.clone().unwrap_or_default();
    wrapper.mask_clip_off = c.mask_clip.filter(|k| *k != 255).map(|k| box_off(Some(k)));
    // SVG-ребёнок: коробки маски уже посчитаны от его stroke-box
    // (`svg::masked_layers`), рамки и отбивки у него нет.
    if let Some((origin, clip)) = c.mask_box_override {
        wrapper.mask_origin_off = origin;
        wrapper.mask_clip_off = clip;
    }
    if c.mask_user_scale > 0.0 {
        wrapper.mask_scale = c.mask_user_scale;
    }
    // Точки уходят КАК ЕСТЬ (Len): проценты и пиксели резолвятся при
    // отрисовке от опорной коробки формы (css-masking §1.3.1.1): margin-box
    // расширяет bounds на поля, content-box сужает на рамку+паддинг
    // (clip-path-polygon-008: полигон в margin-box; masking 82→84).
    wrapper.polygon = polygon.to_vec();
    wrapper.polygon_evenodd = c.clip_polygon_evenodd;
    let side = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let b = c.borders();
    wrapper.poly_expand = match c.clip_ref {
        Some(1) => [
            side(c.margin.top),
            side(c.margin.right),
            side(c.margin.bottom),
            side(c.margin.left),
        ],
        Some(2) => [-side(b.top), -side(b.right), -side(b.bottom), -side(b.left)],
        Some(3) => [
            -(side(b.top) + side(c.padding.top)),
            -(side(b.right) + side(c.padding.right)),
            -(side(b.bottom) + side(c.padding.bottom)),
            -(side(b.left) + side(c.padding.left)),
        ],
        _ => [0.0; 4],
    };
    wrapper.into_any_element()
}
