//! Цепочки полей первого и последнего ребёнка, нули и закреплённые поля.

use super::through::ThroughCache;
use super::through::adjoin;
use super::through::float_only_wrapper;
use super::through::strut_of;
use super::{Strut, inline_axis_edges, margin_or_bail, top_edge_open_at};
use crate::dom::Node;
use crate::dom::whitespace::blank_text;
use crate::layout::block::containing::with_inner_dimensions;
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::box_kind::{atomic_inline, in_flow, inline_marked_block, replaced_inline};
use crate::layout::float::clear::bfc_no_fit;
use crate::layout::float::initial_letter::px_margin_w;
use crate::style::values::value::Len;
mod pins;
pub(crate) use pins::margin_px;
pub(crate) use pins::pin_inherited_margins;
mod zero;
pub(crate) use zero::{ZeroPaths, zero_paths};

pub(crate) fn leading_chain(
    children: &[Node],
    eat: &mut ZeroPaths,
    cb: Option<f32>,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<Strut> {
    let mut cache = ThroughCache::default();
    let mut parent = 0;
    let mut children = children;
    let mut cb = cb;
    let mut vcx = vcx;
    let mut s = strut_of(0.0);
    'level: loop {
        // Нижняя оценка ширины флоатов, которые цепь уже прошла: они ПРИМЫКАЮТ к
        // полю следующего в потоке (их блочное смещение ещё не решено).
        let mut adjoining_floats: Option<f32> = None;
        let (mut adjoining_left, mut adjoining_right) = (false, false);
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
                if ch.children.is_empty()
                    && !replaced_inline(&ch.tag)
                    && !inline_axis_edges(&ch.style.used())
                {
                    continue;
                }
                return Some(s);
            }
            // Атомарный строчный в потоке есть и рождает строчную коробку.
            if atomic_inline(&ch.style.used()) {
                return Some(s);
            }
            // Плавающий и абсолютный в потоке не участвуют и примыкания не рвут.
            if !in_flow(&ch.style.used()) {
                if ch.style.used().float.is_some_and(|f| f != 0) {
                    adjoining_left |= ch.style.used().float.is_some_and(|f| f < 0);
                    adjoining_right |= ch.style.used().float.is_some_and(|f| f > 0);
                    let w = px_margin_w(&ch.style.used()).unwrap_or(0.0);
                    adjoining_floats = Some(adjoining_floats.map_or(w, |v| v.min(w)));
                }
                continue;
            }
            // Клиренс разделяет поля (§8.3.1): дальше по цепи примыкание не
            // идёт, и поле такого ребёнка наружу не уходит.
            // `clear` alone is not clearance. A non-adjoining float may already
            // be above the hypothetical border edge after margin collapse.
            // Blink HasClearancePastAdjoiningFloats tests the cleared side.
            // Keep definite-height boxes on the existing clearance path: their
            // external float offset cannot be resolved by this margin-only walk.
            if ch.style.used().clear.is_some_and(|side| {
                ch.style.used().height.is_some()
                    || (side <= 0 && adjoining_left)
                    || (side >= 0 && adjoining_right)
            }) {
                return Some(s);
            }
            // Коробка своего контекста, которой рядом с ПРИМЫКАЮЩИМИ флоатами нет
            // места (§9.5), отделяется от них, как клиренсом: её поле флоат вниз
            // не увозит (ассерт `new-fc-separates-from-float`: «will need to
            // separate its margin from the float, so that it doesn't affect the
            // float»). Дальше по цепи примыкание не идёт.
            if adjoining_floats.is_some_and(|w| bfc_no_fit(ch, w, cb)) {
                return Some(s);
            }
            // Поле САМОГО ребёнка примыкает к полю родителя всегда — даже когда
            // ребёнок заводит свой контекст: запрет §8.3.1 лежит на РОДИТЕЛЕ.
            s = adjoin(
                s,
                strut_of(margin_or_bail(
                    ch.style.used().margin.top,
                    &ch.style.used(),
                    vcx,
                )?),
            );

            if let Some(t) = cache.get(ch, false, vcx) {
                // Насквозь: поля всего поддерева уже в струне, идём к брату.
                s = adjoin(s, t);
                eat.add(parent, i, true);

                if let Some((left, right, w)) = float_only_wrapper(ch) {
                    adjoining_left |= left;
                    adjoining_right |= right;
                    adjoining_floats = Some(adjoining_floats.map_or(w, |v| v.min(w)));
                }
                continue;
            }
            let next = eat.add(parent, i, false);
            // Спуск в ребёнка с `margin-trim: block-start` не нужен: верхнее поле
            // его первого ребёнка (и всё, что с ним схлопнулось) обрезано, наружу
            // примыкать нечему. Без этого поле 50 ребёнка уходило через контейнер
            // в `body`, и страница съезжала на 50 (`block-container-non-adjoining-item`).
            if top_edge_open_at(ch, vcx.collapse.width) && ch.style.used().margin_trim & 1 == 0 {
                // Содержащий блок детей `ch` — сам `ch`: его ширина точками —
                // оценка для `bfc_no_fit` на спуске.
                cb = match ch.style.used().width {
                    Some(Len::Px(w)) if w > 0.0 => Some(w),
                    _ => cb,
                };
                vcx = with_inner_dimensions(
                    &ch.style.used(),
                    &BoxDimensions::for_element(ch, &ch.style.used()),
                    vcx,
                    |inner| inner,
                );
                children = &ch.children;
                parent = next;
                continue 'level;
            }

            return Some(s);
        }
        return Some(s);
    }
}

/// Хвостовая цепочка примыкающих НИЖНИХ полей — зеркало `leading_chain`.
///
/// В схлопывании она НЕ участвует: поднимать её наружу нельзя —
/// вглубь подъём уходил мимо ещё неизвестной высоты родителя. Для
/// `margin-trim: block-end` этой опасности нет: наружу ничего не поднимается,
/// поля только гасятся, и цепочка ограничена последним ребёнком в потоке.
pub(crate) fn trailing_chain(
    children: &[Node],
    eat: &mut ZeroPaths,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<Strut> {
    let mut cache = ThroughCache::default();
    let mut parent = 0;
    let mut children = children;
    let mut s = strut_of(0.0);
    'level: loop {
        for (i, c) in children.iter().enumerate().rev() {
            let ch = match c {
                Node::Text(t) if blank_text(t) => continue,
                Node::Text(_) => return Some(s),
                Node::Element(ch) => ch,
            };
            if ch.inline && !inline_marked_block(ch) {
                if ch.children.is_empty()
                    && !replaced_inline(&ch.tag)
                    && !inline_axis_edges(&ch.style.used())
                {
                    continue;
                }
                return Some(s);
            }
            if atomic_inline(&ch.style.used()) {
                return Some(s);
            }
            if !in_flow(&ch.style.used()) {
                continue;
            }
            s = adjoin(
                s,
                strut_of(margin_or_bail(
                    ch.style.used().margin.bottom,
                    &ch.style.used(),
                    vcx,
                )?),
            );

            if let Some(t) = cache.get(ch, false, vcx) {
                s = adjoin(s, t);
                eat.add(parent, i, true);

                continue;
            }
            let next = eat.add(parent, i, false);
            // Заданная высота или нижняя рамка/отступ отрезают цепочку: поле
            // внука к краю контейнера уже не примыкает.
            if ch.style.used().height.is_none() && top_edge_open_at(ch, vcx.collapse.width) {
                children = &ch.children;
                parent = next;
                continue 'level;
            }

            return Some(s);
        }
        return Some(s);
    }
}
