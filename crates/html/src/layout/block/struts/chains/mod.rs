//! Цепочки полей первого и последнего ребёнка, нули и закреплённые поля.

use super::through::adjoin;
use super::through::float_only_wrapper;
use super::through::strut_of;
use super::through::through_strut;
use super::{Strut, inline_axis_edges, margin_or_bail, top_edge_open};
use crate::dom::Node;
use crate::layout::block::containing::{CB_WIDTH, with_inner_cb};
use crate::layout::float::clear::bfc_no_fit;
use crate::layout::float::initial_letter::px_margin_w;
use crate::render::{atomic_inline, in_flow, inline_marked_block, replaced_inline};
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;
mod pins;
pub(crate) use pins::margin_px;
pub(crate) use pins::pin_inherited_margins;
pub(crate) use pins::zero_at;

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
