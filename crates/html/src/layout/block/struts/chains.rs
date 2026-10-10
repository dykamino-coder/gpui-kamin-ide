//! Цепочки полей первого и последнего ребёнка, нули и закреплённые поля.

use super::through::adjoin;
use super::through::float_only_wrapper;
use super::through::strut_of;
use super::through::through_strut;
use super::{Strut, inline_axis_edges, margin_or_bail, top_edge_open};
use crate::dom::{Element, Node};
use crate::layout::block::containing::{CB_WIDTH, with_inner_cb};
use crate::layout::block::margins::{COLLAPSE_CB_WIDTH_PX, COLLAPSE_FONT_PX};
use crate::layout::float::clear::bfc_no_fit;
use crate::layout::float::initial_letter::px_margin_w;
use crate::render::{atomic_inline, in_flow, inline_marked_block, replaced_inline};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;

pub(crate) fn leading_chain(
    children: &[Node],
    path: &mut Vec<usize>,
    eat: &mut Vec<(Vec<usize>, bool)>,
) -> Option<Strut> {
    let mut s = strut_of(0.0);
    // Нижняя оценка ширины флоатов, которые цепь уже прошла: они ПРИМЫКАЮТ к
    // полю следующего в потоке (их блочное смещение ещё не решено).
    let mut adjoining_floats: Option<f32> = None;
    for (i, c) in children.iter().enumerate() {
        let ch = match c {
            Node::Text(t) if blank_text(t) => continue,
            // Непробельный текст — строчная коробка, примыкание кончилось.
            Node::Text(_) => return Some(s),
            Node::Element(ch) => ch,
        };
        // Блочный псевдоэлемент (`::after { display: flow-root }`) —
        // блочный ребёнок, его поле примыкает; см. `inline_marked_block`.
        if ch.inline && !inline_marked_block(ch) {
            // Пустой `<span>` прозрачен, замещаемый атом рождает строку;
            // пустой строчный с полем/отступом/рамкой по строчной оси — не
            // фантом, строка есть (css-inline-3 §invisible-line-boxes).
            if ch.children.is_empty() && !replaced_inline(&ch.tag) && !inline_axis_edges(&ch.style)
            {
                continue;
            }
            return Some(s);
        }
        // Атомарный строчный в потоке есть и рождает строчную коробку.
        if atomic_inline(&ch.style) {
            return Some(s);
        }
        // Плавающий и абсолютный в потоке не участвуют и примыкания не рвут.
        if !in_flow(&ch.style) {
            if ch.style.float.is_some_and(|f| f != 0) {
                let w = px_margin_w(&ch.style).unwrap_or(0.0);
                adjoining_floats = Some(adjoining_floats.map_or(w, |v| v.min(w)));
            }
            continue;
        }
        // Клиренс разделяет поля (§8.3.1): дальше по цепи примыкание не
        // идёт, и поле такого ребёнка наружу не уходит.
        if ch.style.clear.is_some() {
            return Some(s);
        }
        // Коробка своего контекста, которой рядом с ПРИМЫКАЮЩИМИ флоатами нет
        // места (§9.5), отделяется от них, как клиренсом: её поле флоат вниз
        // не увозит (ассерт `new-fc-separates-from-float`: «will need to
        // separate its margin from the float, so that it doesn't affect the
        // float»). Дальше по цепи примыкание не идёт.
        if adjoining_floats.is_some_and(|w| bfc_no_fit(ch, w)) {
            return Some(s);
        }
        // Поле САМОГО ребёнка примыкает к полю родителя всегда — даже когда
        // ребёнок заводит свой контекст: запрет §8.3.1 лежит на РОДИТЕЛЕ.
        s = adjoin(s, strut_of(margin_or_bail(ch.style.margin.top, &ch.style)?));
        path.push(i);
        if let Some(t) = through_strut(ch) {
            // Насквозь: поля всего поддерева уже в струне, идём к брату.
            s = adjoin(s, t);
            eat.push((path.clone(), true));
            path.pop();
            if let Some((_, _, w)) = float_only_wrapper(ch) {
                adjoining_floats = Some(adjoining_floats.map_or(w, |v| v.min(w)));
            }
            continue;
        }
        eat.push((path.clone(), false));
        // Спуск в ребёнка с `margin-trim: block-start` не нужен: верхнее поле
        // его первого ребёнка (и всё, что с ним схлопнулось) обрезано, наружу
        // примыкать нечему. Без этого поле 50 ребёнка уходило через контейнер
        // в `body`, и страница съезжала на 50 (`block-container-non-adjoining-item`).
        if top_edge_open(ch) && ch.style.margin_trim & 1 == 0 {
            // Содержащий блок детей `ch` — сам `ch`: его ширина точками —
            // оценка для `bfc_no_fit` на спуске.
            let cb_prev = CB_WIDTH.get();
            if let Some(Len::Px(w)) = ch.style.width
                && w > 0.0
            {
                CB_WIDTH.set(Some(w));
            }
            // Внуки меряют доли от содержимого `ch`, а не от уровня (§10.1 п.2).
            let inner = with_inner_cb(&ch.style, || leading_chain(&ch.children, path, eat));
            CB_WIDTH.set(cb_prev);
            s = adjoin(s, inner?);
        }
        path.pop();
        return Some(s);
    }
    Some(s)
}

/// Хвостовая цепочка примыкающих НИЖНИХ полей — зеркало `leading_chain`.
///
/// В схлопывании она НЕ участвует: попытка поднимать её наружу замерена и
/// откачена (см. комментарий в `collapse_margins`, CSS2 4684 -> 4594) —
/// вглубь подъём уходил мимо ещё неизвестной высоты родителя. Для
/// `margin-trim: block-end` этой опасности нет: наружу ничего не поднимается,
/// поля только гасятся, и цепочка ограничена последним ребёнком в потоке.
pub(crate) fn trailing_chain(
    children: &[Node],
    path: &mut Vec<usize>,
    eat: &mut Vec<(Vec<usize>, bool)>,
) -> Option<Strut> {
    let mut s = strut_of(0.0);
    for (i, c) in children.iter().enumerate().rev() {
        let ch = match c {
            Node::Text(t) if blank_text(t) => continue,
            Node::Text(_) => return Some(s),
            Node::Element(ch) => ch,
        };
        if ch.inline && !inline_marked_block(ch) {
            if ch.children.is_empty() && !replaced_inline(&ch.tag) && !inline_axis_edges(&ch.style)
            {
                continue;
            }
            return Some(s);
        }
        if atomic_inline(&ch.style) {
            return Some(s);
        }
        if !in_flow(&ch.style) {
            continue;
        }
        s = adjoin(
            s,
            strut_of(margin_or_bail(ch.style.margin.bottom, &ch.style)?),
        );
        path.push(i);
        if let Some(t) = through_strut(ch) {
            s = adjoin(s, t);
            eat.push((path.clone(), true));
            path.pop();
            continue;
        }
        eat.push((path.clone(), false));
        // Заданная высота или нижняя рамка/отступ отрезают цепочку: поле
        // внука к краю контейнера уже не примыкает.
        if ch.style.height.is_none() && top_edge_open(ch) {
            s = adjoin(s, trailing_chain(&ch.children, path, eat)?);
        }
        path.pop();
        return Some(s);
    }
    Some(s)
}

/// Обнулить поле по пути: наружу оно ушло одним полем родителя, и раскладка
/// сложила бы его второй раз. `deep` — коробка схлопнулась насквозь: чистится
/// она сама с обеих сторон.
pub(crate) fn zero_at(children: &mut [Node], path: &[usize], top: bool, deep: bool) {
    let Some((&i, rest)) = path.split_first() else {
        return;
    };
    let Some(Node::Element(ch)) = children.get_mut(i) else {
        return;
    };
    if !rest.is_empty() {
        zero_at(&mut ch.children, rest, top, deep);
        return;
    }
    pin_inherited_margins(ch, top || deep, !top || deep);
    if deep {
        ch.style.margin.top = Some(Len::Px(0.0));
        ch.style.margin.bottom = Some(Len::Px(0.0));
    } else if top {
        ch.style.margin.top = Some(Len::Px(0.0));
    } else {
        ch.style.margin.bottom = Some(Len::Px(0.0));
    }
}

/// `margin: inherit` берёт ВЫЧИСЛЕННОЕ поле родителя (CSS 2.1 §6.2.1: «the
/// property takes the same computed value as the property for the element's
/// parent»). Схлопывание же переписывает поле родителя (обнуляет ушедшее
/// наружу, пишет остаток струны) РАНЬШЕ, чем до ребёнка доходит наследование
/// (`inline::inherit` живёт ниже по пути), — и ребёнок наследовал used-ноль
/// вместо написанного: `margin-bottom-113` (низ `#wrapper` ушёл в `body`),
/// `margin-top-113` (остаток 80 вместо 96), `margin-em-inherit-001` (верх
/// `#parent` съеден цепочкой `#grand-parent`). Поэтому до перезаписи
/// наследующие дети получают поле как есть и флаг снимается: шрифтовые
/// единицы — в точки по кеглю РОДИТЕЛЯ (наследуется вычисленная длина, «not
/// 80px 120px 40px 160px»), доля остаётся долей и решается от своего
/// содержащего блока (`margin-percentage-inherit-001`: 15% от 200, а не 60).
pub(crate) fn pin_inherited_margins(e: &mut Element, top: bool, bottom: bool) {
    let own = e.style.margin;
    for n in e.children.iter_mut() {
        let Node::Element(ch) = n else { continue };
        for (i, want, src) in [(0usize, top, own.top), (2, bottom, own.bottom)] {
            if !want || !ch.style.margin_inherit[i] {
                continue;
            }
            let pinned = match src {
                Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_))) => {
                    margin_px(Some(l), &e.style).map(Len::Px).or(Some(l))
                }
                other => other,
            };
            if i == 0 {
                ch.style.margin.top = pinned;
            } else {
                ch.style.margin.bottom = pinned;
            }
            ch.style.margin_inherit[i] = false;
        }
    }
}

/// Отступ в точках для схлопывания.
///
/// Схлопывание идёт ДО каскада размеров шрифта, а разметка пишет `margin: 1em 0`
/// не реже, чем в точках: без перевода правило не срабатывало ни разу на таких
/// отступах, и блоки уезжали вниз на целый отступ. Кегль берётся свой, если
/// элемент его задал, иначе базовый — унаследованного здесь ещё нет.
/// Проценты не переводятся: они считаются от ширины родителя, а её тут никто
/// не знает, и выдуманное число было бы хуже пропуска.
pub(crate) fn margin_px(l: Option<Len>, style: &Computed) -> Option<f32> {
    // Кегль элемента: свой, если задан, иначе унаследованный от уровня
    // (см. `COLLAPSE_FONT_PX`). Прежде вместо унаследованного брались
    // постоянные 16 точек, и `table{font-size:50px} div{margin:1em 0}`
    // схлопывался по 16 вместо 50 — вся семья Hixie `margin-collapse-1xx`
    // расходилась с эталоном ровно на эту разницу.
    let parent = COLLAPSE_FONT_PX.with(std::cell::Cell::get);
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
        // (`inline::inherit`), а схлопывание идёт раньше. Прежде `-6ex`
        // отдавало `None`, поле пропадало целиком (`positioning/top-091`).
        l @ (Len::Ch(_) | Len::Ex(_)) => Some(crate::text::metrics::spacing_px(
            Some(l),
            &style.font_family.clone().unwrap_or_default(),
            base,
        )),
        // Процент — от ширины содержащего блока, когда она известна в точках
        // (`margin-top-103`, `margin-bottom-113`); иначе поле пропускается.
        Len::Pct(k) => COLLAPSE_CB_WIDTH_PX
            .with(std::cell::Cell::get)
            .map(|w| k * w),
        _ => None,
    }
}
