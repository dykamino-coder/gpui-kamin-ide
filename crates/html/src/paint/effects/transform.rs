//! Обёртка `Transformed` по вычисленному стилю: `transform` с отдельными `rotate`/`scale`
//! (css-transforms-2 §ctm), `transform-origin`, `perspective` родителя и своя, ячейки объёмного
//! контекста `preserve-3d` и `backface-visibility`; групповые свойства сплющивают контекст
//! (§grouping-property-values), `inherit` решается от того же родителя с учётом `zoom`,
//! чистый px-сдвиг уже учтён раскладкой. На вход — собранный элемент, свой и родительский стиль.

use crate::hooks::IntoHooked;
use crate::style::computed::Computed;
use gpui::AnyElement;

/// css-transforms-2 §grouping-property-values: «групповые» свойства делают
/// из элемента группу, и ИСПОЛЬЗУЕМОЕ значение `transform-style` у него —
/// `flat`, чем бы ни было записано. Без гейта зелёные
/// `preserve3d-and-filter-no-perspective` (filter),
/// `transform3d-preserve3d-009` (overflow),
/// `mix-blend-mode-with-transform-and-preserve-3D` (blend),
/// `clip-not-absolute-positioned-003`, `corner-shape-bevel-overflow-composite`
/// и `view-transition-name-is-grouping` уходят в красное.
fn flattens_3d(c: &Computed) -> bool {
    use crate::style::computed::Overflow;
    let clipped = |o: Option<Overflow>| matches!(o, Some(o) if o != Overflow::Visible);
    c.opacity.is_some_and(|o| o < 1.0)
        || c.filter.is_some()
        || c.filter_ref.is_some()
        || c.backdrop_blur.is_some()
        || clipped(c.overflow_x)
        || clipped(c.overflow_y)
        || c.mask_image.is_some()
        || c.clip_ref.is_some()
        || c.blend.is_some_and(|b| b != 0)
        || c.isolate == Some(true)
        || c.contain_paint == Some(true)
        || c.contain_layout == Some(true)
}

pub(crate) fn transformed(
    el: AnyElement,
    c: &Computed,
    parent: &Computed,
    vcx: crate::document::state::ValueCx<'_>,
) -> AnyElement {
    transformed_with(el, c, parent, None, vcx)
}

/// `transformed` with a shared reference box (`transformed_element::Transformed::ref_box`):
/// a table row or row group transform spread over its cells.
pub(crate) fn transformed_with(
    el: AnyElement,
    c: &Computed,
    parent: &Computed,
    ref_box: Option<crate::layout::build_contracts::RefBox>,
    vcx: crate::document::state::ValueCx<'_>,
) -> AnyElement {
    // `transform: inherit` / `transform-origin: inherit` (css-cascade-4
    // §inherit: «the property's specified and computed values are the
    // inherited value»). Разбор ставит только бит, а значение родителя
    // кладёт `cascade::inherit` в СЛИТЫЙ стиль. Блочный путь строит
    // обёртку по `&e.style`, и
    // унаследованное значение терялось (`transform-inherit-001/002`,
    // `-origin-001/002`, `css-transform-inherit-scale`). Бит решается здесь,
    // от того же родителя; у формы (`&merged`) результат тот же.
    let inherited_tf;
    let c = {
        use crate::style::computed::inh;
        let bits = c.inherit_bits & (inh::TRANSFORM | inh::TRANSFORM_ORIGIN);
        if bits == 0 {
            c
        } else {
            let mut own = c.clone();
            // Значение родителя уже в его зуме: домножаем на отношение
            // собственного зума к родительскому (css-viewport-1 §493).
            let k = c.zoom_eff.unwrap_or(1.0) / parent.zoom_eff.unwrap_or(1.0);
            if bits & inh::TRANSFORM != 0 {
                own.transform = parent.transform;
                if let Some(t) = own.transform.as_mut()
                    && (k - 1.0).abs() > f32::EPSILON
                {
                    crate::style::zoom::scale_transform(t, k);
                }
            }
            if bits & inh::TRANSFORM_ORIGIN != 0 {
                own.transform_origin = parent.transform_origin;
                own.transform_origin_px = parent.transform_origin_px;
                own.transform_origin_z = parent.transform_origin_z;
                own.transform_origin_px.0 = own.transform_origin_px.0.map(|v| v * k);
                own.transform_origin_px.1 = own.transform_origin_px.1.map(|v| v * k);
                own.transform_origin_z = own.transform_origin_z.map(|v| v * k);
            }
            inherited_tf = own;
            &inherited_tf
        }
    };
    // Объёмный контекст: своя ячейка нужна владельцу `preserve-3d`, чужая —
    // КАЖДОМУ его прямому ребёнку, даже без собственного `transform`:
    // изнанка решается по НАКОПЛЕННОЙ матрице (`backface-visibility-hidden-004`
    // — у `.card.front` своего преобразования нет вовсе).
    let keeps_3d = c.preserve_3d == Some(true) && !flattens_3d(c);
    // Вынесенный блок-в-строчном (`Computed::hoisted_block`): по DOM он
    // внук, и плоский строчный хозяин — лист контекста: ни ячейка объёма, ни
    // перспектива деда ему не достаются (css-transforms-2
    // §3d-rendering-context; `3d-rendering-context-and-inline`:
    // `rotateX(-90deg)` внутри `display: inline` под `preserve-3d; rotateX(90deg)`
    // не раскручивается обратно; `perspective-children-only-inline`).
    let under_3d = if c.hoisted_block {
        None
    } else {
        parent.frame_3d.clone()
    };
    if c.transform.is_none() && c.perspective.is_none() && !keeps_3d && under_3d.is_none() {
        return el;
    }
    let mut wrapper = crate::paint::effects::transformed_element::Transformed::new(el, vcx);
    wrapper.ref_box = ref_box;
    wrapper.under_3d = under_3d;
    wrapper.frame_3d = if keeps_3d { c.frame_3d.clone() } else { None };
    // Перспектива РОДИТЕЛЯ читается объёмным путём (css-transforms-2
    // §3d-transform-rendering п.3 — только прямого родителя, внукам не
    // достаётся: perspective-children-only-*); своя — наполняет ячейку для
    // детей. Элементу с `perspective` без `transform` обёртка тоже нужна —
    // ради ячейки; сам он идёт плоским путём с единичной матрицей.
    // Известные пробелы: `perspective-children-only-inline` — блок внутри
    // `display: inline` выносится расщеплением строчного (block-in-inline) и
    // становится прямым ребёнком в дереве отрисовки, поэтому берёт
    // перспективу, хотя по DOM он внук; `overflow-perspective-001` —
    // у прокручиваемой коробки начало перспективы считается от коробки,
    // а не от области прокрутки.
    wrapper.under_perspective = if c.hoisted_block {
        None
    } else {
        parent.perspective_frame.clone()
    };
    wrapper.perspective = c.perspective;
    wrapper.perspective_frame = c.perspective_frame.clone();
    if let Some(o) = c.perspective_origin {
        wrapper.perspective_origin = o;
    }
    wrapper.perspective_origin_px = c.perspective_origin_px;
    // Изнанка ставится ДО раннего выхода: ребёнок объёмного контекста без
    // своего `transform` (`backface-visibility-hidden-004` `.card.front`,
    // `transform3d-backface-visibility-006`, `backface-visibility-with-
    // sibling-001`) решает её по накопленной матрице родителя
    // (css-transforms-2 §backface-visibility), а флаг прежде выставлялся
    // только на пути с собственным преобразованием — красный ребёнок под
    // `rotateX(180deg); preserve-3d` оставался виден.
    wrapper.backface_hidden = c.backface_hidden == Some(true);
    wrapper.fragment_clip_ancestor = c.fragment_clip_ancestor;
    let Some(t) = c.transform else {
        return wrapper.hooked();
    };
    // Отдельные `rotate`/`scale` — ПЕРЕД списком `transform` (css-transforms-2
    // §ctm п.4-5 и п.7; Blink `ComputedStyle::ApplyTransform`,
    // style/computed_style.cc:1464-1487). Разбор писал их только в разложение
    // (`rotate_rad`/`scale`), а `Transformed` рисует по `lin`/`tr`/`m4` —
    // до экрана они не доходили вовсе. `translate` здесь не нужен: он уже
    // сдвинул коробку (`paint/apply/`).
    let mut t = t.after_individual(c.rotate_prop, c.scale_prop);
    // Чистый px-сдвиг уже сдвинул коробку в раскладке
    // (`Computed::folded_shift`, `paint/apply/`) — матрица остаётся единичной.
    if c.folded_shift().is_some() {
        t.tr[0][0] = 0.0;
        t.tr[1][0] = 0.0;
        t.m4[0][3] = 0.0;
        t.m4[1][3] = 0.0;
        t.translate = (0.0, 0.0);
    }
    wrapper.rotate = t.rotate_rad;
    wrapper.skew = t.skew_rad;
    wrapper.scale = t.scale;
    wrapper.translate = t.translate;
    wrapper.translate_pct = t.translate_pct;
    wrapper.lin = t.lin;
    wrapper.tr = t.tr;
    wrapper.m4 = t.m4;
    wrapper.m4_pct = t.m4_pct;
    wrapper.has_3d = t.has_3d;
    // Обратная сторона (css-transforms-2 §backface-visibility, «m33 < 0 →
    // the element is not rendered») решается на отрисовке по собственной
    // 4×4 (флаг выставлен выше): раньше здесь подменяли элемент пустым
    // `div()`, и коробка теряла место в раскладке
    // (backface-visibility-hidden-002: эталон держит пустые 100px;
    // -child-translate: высота обёртки от скрытого ребёнка).
    if let Some(o) = c.transform_origin {
        wrapper.origin = o;
    }
    wrapper.origin_px = c.transform_origin_px;
    wrapper.origin_z = c.transform_origin_z;
    wrapper.hooked()
}
