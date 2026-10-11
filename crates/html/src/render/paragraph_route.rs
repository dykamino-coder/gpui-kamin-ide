//! Сборка абзаца с пробами и слоями inline-окраски.

use super::RenderOpts;
use super::paragraph::paragraph;
use crate::dom::Node;
use crate::paint::decorations::text_shadows::with_text_shadow;
use crate::style::computed::Computed;
use crate::text::text_box::line_height_px;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

/// Разбор списка детей на блоки: инлайн-подряд склеивается в абзац.
/// Абзац с пробой бюджета строк: если строится внутри clamp-контейнера,
/// рядом с абзацем едет проба его границ и высоты строки.
/// Строчное содержимое блочного контейнера рисуется на шаге 7 приложения E
/// CSS 2.1 — после фонов и рамок ВСЕХ блоков потока своего контекста
/// наложения (шаг 4) и флоатов (шаг 5), в порядке дерева, но до
/// позиционированных (шаг 8). Обёртка раскладку не меняет: при открытом
/// собирателе краски (`gpui::PaintCollect`) абзац уходит в него, иначе
/// рисуется на месте (`gpui::PaintInline`).
pub(super) fn paint_inline_step7(para: AnyElement) -> AnyElement {
    gpui::PaintInline::new(para).into_any_element()
}

pub(super) fn paragraph_probed(
    taken: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
) -> AnyElement {
    // Знак обрыва АВТО-режима: бюджет строк ИМЕННО ЭТОГО абзаца посчитал
    // `ClampCut` прошлого кадра. Кладём его ДО сборки абзаца — многоточие
    // нарисует строчный слой (`lines::clamp_lines` → `paint_line`), тот
    // самый, что уже зелен на `webkit-line-clamp-014` (bidi) 0.00,
    // `block-ellipsis-bidi-001/002` 0.00, `block-ellipsis-028/031` 0.19.
    // Номер абзаца выдаётся в порядке ПОСТРОЕНИЯ и уезжает в пробу,
    // поэтому сопоставление кадров не зависит от порядка обхода на
    // отрисовке.
    let ctx = crate::text::clamp::clamp_context();
    let seq = ctx.map(|(key, _)| crate::text::clamp::clamp_next_seq(key));
    let budget = match (ctx, seq) {
        (Some((key, _)), Some(s)) => match crate::text::clamp::clamp_para(key) {
            Some((ps, k)) if ps == s => Some(k),
            _ => None,
        },
        _ => None,
    };
    crate::text::clamp::set_para_budget(budget);
    crate::text::clamp::set_para_tag(ctx.zip(seq).map(|((key, _), s)| (key, s)));
    let para = paragraph(taken, inherited, opts);
    crate::text::clamp::set_para_tag(None);
    // Ячейку обязательно опустошить и когда абзац её не забрал
    // (вертикальное письмо уходит из `paragraph` раньше): иначе бюджет
    // достался бы СЛЕДУЮЩЕМУ абзацу.
    crate::text::clamp::set_para_budget(None);
    let para = with_text_shadow(para, inherited, taken, opts);
    if let Some((key, skip)) = ctx {
        div()
            .relative()
            .child(para)
            .child(crate::text::clamp::clamp_probe(
                crate::text::clamp::clamp_lines_for(key),
                line_height_px(inherited, opts),
                skip,
                false,
                0.0,
                seq,
                budget,
            ))
            .into_any_element()
    } else {
        para
    }
}
