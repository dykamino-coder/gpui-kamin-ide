//! Струна примыкающих полей соседних блоков одного потока (CSS 2.1 §8.3.1): второй проход
//! `collapse_margins` после подъёма полей детей. Для каждого блочного ребёнка слитое поле
//! решается из струны и уже записанных точек (`emitted`), верхнему полю достаётся разница,
//! насквозь-схлопнутая коробка отдаёт свои поля струне, коробка с клиренсом оставляет
//! остаток внутри родителя (`cleared_run`), флоаты запоминают поле для полос обтекания.
//! Строчная коробка с содержимым и непустой текст струну обрывают.

use crate::dom::Node;
use crate::layout::block::struts::{
    adjoin, margin_px, pin_inherited_margins, solve, strut_of, through_strut,
    through_strut_no_clear,
};
use crate::layout::box_kind::{in_flow, inline_level_box};
use crate::layout::float::band_clearance;
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;

pub(crate) fn emit_margin_struts(
    out: &mut [Node],
    strut: &mut Option<(f32, f32)>,
    emitted: &mut f32,
    cleared_run: &mut Option<usize>,
    vcx: crate::document::state::ValueCx<'_>,
) {
    for (idx, node) in out.iter_mut().enumerate() {
        let Node::Element(e) = node else {
            // Переводы строк между блоками разрывом потока не считаются: в
            // форматированной разметке они стоят везде, и из-за них
            // схлопывание не срабатывало ни разу.
            if matches!(node, Node::Text(t) if blank_text(t)) {
                continue;
            }
            *strut = None;
            continue;
        };
        // Строчный элемент С СОДЕРЖИМЫМ порождает строчную коробку, и поля
        // блоков через неё уже не примыкают.
        if inline_level_box(e) {
            if !e.children.is_empty() {
                *strut = None;
            }
            continue;
        }
        // Не обрывать струну на атомарном строчном, хотя `inline-block` и
        // родня в потоке рождают строчную коробку: в `bidi-box-model-*` такой
        // сосед стоит между блоками сплошь, и разрыв струны разводит их
        // полями врозь. Вернуть можно только с настоящей строчной коробкой.
        if !in_flow(e.style.cascaded()) {
            band_clearance::remember_float_margin(e, *strut, *emitted);
            continue;
        }
        let top = margin_px(e.style.cascaded().margin.top, e.style.cascaded(), vcx).unwrap_or(0.0);
        let bottom =
            margin_px(e.style.cascaded().margin.bottom, e.style.cascaded(), vcx).unwrap_or(0.0);
        let through = through_strut(e, vcx);
        let mut merged = match *strut {
            Some(s) => {
                let m = adjoin(s, strut_of(top));
                // Верхний край насквозь-схлопнутой коробки встаёт там, где
                // разрешается струна до неё вместе с её верхним полем. Та же
                // строка даёт это и обычной коробке: нижнее поле соседа
                // раскладка уже поставила, верхнему достаётся разница.
                pin_inherited_margins(e, true, false, vcx);
                e.style.cascaded_mut().margin.top = Some(Len::Px(solve(m) - *emitted));
                *emitted = solve(m);
                m
            }
            None => {
                // Примыкать не к чему: верхнее поле остаётся как написано.
                *emitted = top;
                strut_of(top)
            }
        };
        // Коробка с клиренсом, которая иначе схлопнулась бы насквозь
        // (§8.3.1): её поля СЛИВАЮТСЯ между собой, но получившееся поле не
        // схлопывается с нижним полем родителя — «these margins collapse with
        // the adjoining margins of following siblings but the resulting margin
        // does not collapse with the bottom margin of the parent block».
        // Верхнее поле уже выложено рядом обтекания, поэтому наружу идёт
        // только остаток.
        if through.is_none()
            && e.style.cascaded().clear.is_some()
            && through_strut_no_clear(e, vcx).is_some()
        {
            *emitted = top;
            pin_inherited_margins(e, false, true, vcx);
            e.style.cascaded_mut().margin.bottom = Some(Len::Px(0.0));
            *strut = Some(adjoin(strut_of(top), strut_of(bottom)));
            *cleared_run = Some(idx);
            continue;
        }
        if let Some(own) = through {
            merged = adjoin(merged, own);
            // Своё нижнее поле коробка не ставит: оно ушло в струну, и
            // раскладка сложила бы его второй раз.
            pin_inherited_margins(e, false, true, vcx);
            e.style.cascaded_mut().margin.bottom = Some(Len::Px(0.0));
            // Не снимать поля и у ДЕТЕЙ насквозь-коробки (`zero_margins_deep`):
            // поля детей уже учтены струной, но раскладка ставит саму коробку
            // не по струне, а по своим полям — снятие уводит её вверх
            // (`floats-clear/margin-collapse-033/034/035`).
            *strut = Some(merged);
            continue;
        }
        *strut = Some(strut_of(bottom));
        *emitted = bottom;
        *cleared_run = None;
    }
}
