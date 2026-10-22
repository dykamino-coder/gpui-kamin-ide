//! Нулевые поля, закреплённые унаследованные поля и поле в точках.

use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Обнулить поле по пути: наружу оно ушло одним полем родителя, и раскладка
/// сложила бы его второй раз. `deep` — коробка схлопнулась насквозь: чистится
/// она сама с обеих сторон.
pub(crate) fn zero_at(
    children: &mut [Node],
    path: &[usize],
    top: bool,
    deep: bool,
    vcx: crate::document::state::ValueCx<'_>,
) {
    let Some((&i, rest)) = path.split_first() else {
        return;
    };
    let Some(Node::Element(ch)) = children.get_mut(i) else {
        return;
    };
    if !rest.is_empty() {
        zero_at(&mut ch.children, rest, top, deep, vcx);
        return;
    }
    pin_inherited_margins(ch, top || deep, !top || deep, vcx);
    if deep {
        ch.style.used_mut().margin.top = Some(Len::Px(0.0));
        ch.style.used_mut().margin.bottom = Some(Len::Px(0.0));
    } else if top {
        ch.style.used_mut().margin.top = Some(Len::Px(0.0));
    } else {
        ch.style.used_mut().margin.bottom = Some(Len::Px(0.0));
    }
}

/// `margin: inherit` берёт ВЫЧИСЛЕННОЕ поле родителя (CSS 2.1 §6.2.1: «the
/// property takes the same computed value as the property for the element's
/// parent»). Схлопывание же переписывает поле родителя (обнуляет ушедшее
/// наружу, пишет остаток струны) РАНЬШЕ, чем до ребёнка доходит наследование
/// (`style::cascade::inherit` живёт ниже по пути), — и ребёнок наследовал used-ноль
/// вместо написанного: `margin-bottom-113` (низ `#wrapper` ушёл в `body`),
/// `margin-top-113` (остаток 80 вместо 96), `margin-em-inherit-001` (верх
/// `#parent` съеден цепочкой `#grand-parent`). Поэтому до перезаписи
/// наследующие дети получают поле как есть и флаг снимается: шрифтовые
/// единицы — в точки по кеглю РОДИТЕЛЯ (наследуется вычисленная длина, «not
/// 80px 120px 40px 160px»), доля остаётся долей и решается от своего
/// содержащего блока (`margin-percentage-inherit-001`: 15% от 200, а не 60).
pub(crate) fn pin_inherited_margins(
    e: &mut Element,
    top: bool,
    bottom: bool,
    vcx: crate::document::state::ValueCx<'_>,
) {
    let own = e.style.computed().margin;
    for n in e.children.iter_mut() {
        let Node::Element(ch) = n else { continue };
        for (i, want, src) in [(0usize, top, own.top), (2, bottom, own.bottom)] {
            let side =
                crate::style::css::LonghandId::physical(crate::style::css::BoxGroup::Margin, i);
            if !want || !ch.style.cascaded().inherit_longhands.contains(side) {
                continue;
            }
            let pinned = match src {
                Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_))) => {
                    margin_px(Some(l), &e.style.computed(), vcx)
                        .map(Len::Px)
                        .or(Some(l))
                }
                other => other,
            };
            if i == 0 {
                ch.style.used_mut().margin.top = pinned;
            } else {
                ch.style.used_mut().margin.bottom = pinned;
            }
            ch.style.used_mut().inherit_longhands.set(side, false);
        }
    }
}

/// Отступ в точках для схлопывания.
///
/// Схлопывание идёт ДО каскада размеров шрифта, а разметка пишет `margin: 1em 0`
/// не реже, чем в точках: без перевода правило не срабатывало ни разу на таких
/// отступах, и блоки уезжали вниз на целый отступ. Кегль берётся свой, если
/// элемент его задал, иначе базовый — унаследованного здесь ещё нет.
/// Доли, в том числе в calc(), требуют известной ширины содержащего блока.
pub(crate) fn margin_px(
    l: Option<Len>,
    style: &Computed,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<f32> {
    // Кегль элемента: свой, если задан, иначе унаследованный от уровня
    // (из входов уровня схлопывания). Прежде вместо унаследованного брались
    // постоянные 16 точек, и `table{font-size:50px} div{margin:1em 0}`
    // схлопывался по 16 вместо 50 — вся семья Hixie `margin-collapse-1xx`
    // расходилась с эталоном ровно на эту разницу.
    let parent = vcx.collapse.font;
    let base = match style.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * parent,
        Some(Len::Pct(k)) => k * parent,
        _ => parent,
    };
    match l? {
        Len::Px(v) => Some(v),
        Len::Em(k) => Some(k * base),
        // Единицы шрифта, считающиеся по МЕТРИКЕ гарнитуры: сюда они доезжают
        // неразрешёнными, потому что `resolve_em` живёт в наследовании
        // (`style::cascade::inherit`), а схлопывание идёт раньше. Прежде `-6ex`
        // отдавало `None`, поле пропадало целиком (`positioning/top-091`).
        l @ (Len::Ch(_) | Len::Ex(_)) => Some(crate::text::metrics::spacing_px(
            Some(l),
            &style.font_family.clone().unwrap_or_default(),
            base,
            vcx,
        )),
        // Процент — от ширины содержащего блока, когда она известна в точках
        // (`margin-top-103`, `margin-bottom-113`); иначе поле пропускается.
        Len::Pct(k) => vcx.collapse.width.map(|w| k * w),
        l @ Len::Calc(_) => crate::style::computed::LenCx {
            basis: vcx.collapse.width,
            ..crate::style::computed::LenCx::box_viewport(Some(vcx.viewport), style)
        }
        .px(l, vcx),
        _ => None,
    }
}
