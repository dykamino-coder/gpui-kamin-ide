//! Границы соседних блоков, поля и перенос запретов в измеренной стопке.
use super::geometry::{BlockBreaks, Pending, walk};
use super::{BlockBox, Laid};

/// Блочные дети стопки: границы соседей, принудительные разрывы и запреты,
/// монолиты и вложенные стопки. Возвращает низ последнего ребёнка.
pub(super) fn walk_kids(
    b: &BlockBox,
    floor: f32,
    laid: &Laid,
    out: &mut BlockBreaks,
) -> Option<f32> {
    let mut prev: Option<(&super::BlockKid, f32, f32)> = None;
    let mut last: Option<f32> = None;
    let mut column_avoid = super::column_avoid::ColumnAvoid::default();
    // Конечное имя страницы последнего именованного соседа.
    let mut page_prev: Option<&str> = None;
    for k in &b.kids {
        let Some((kt, kb)) = laid.at(k.id) else {
            // Коробка без записи раскладки: её места нет, границы вокруг неё
            // не строятся.
            column_avoid.reset();
            prev = None;
            last = None;
            continue;
        };
        column_avoid.visit(kt);
        if !k.adjoins {
            column_avoid.reset();
        }
        let renamed = matches!((page_prev, &k.page), (Some(a), Some((b, _))) if a != b);
        if let Some((_, end)) = &k.page {
            page_prev = Some(end);
        }
        if let Some((p, pt, pb)) = prev.filter(|_| k.adjoins) {
            // Поля flex-элементов не схлопываются и не усекаются разрывом:
            // граница соседних элементов колонки — промежуток между их margin
            // box (css-flexbox-1 §10; у блоков полей в записи нет — border box).
            let pb = laid.margin_box(p.id).map_or(pb, |m| m.1);
            let kt = laid.margin_box(k.id).map_or(kt, |m| m.0);
            // Распорка роста перед коробкой (`STRUT_ID`) — содержимое
            // предыдущего фрагмента: граница встаёт под ней.
            let pb = laid.at(k.id ^ super::STRUT_ID).map_or(pb, |s| s.1.max(pb));
            // Элементы колонки: промежуток у разрыва (row-gap вместе с местом
            // `justify-content`) гасит сдвиг перенесённого элемента к краю
            // фрагментаинера (`push::flex_flow`), а не разрез контейнера —
            // заданный размер контейнера при этом сохраняется.
            let resume = if b.flex_gap.is_some() { pb } else { kt };
            out.cuts.push((pb, resume));
            let (after, before) = (p.edge(true, laid), k.edge(false, laid));
            if after.0 || before.0 || renamed {
                out.forced.push(pb);
                column_avoid.reset();
            } else if (after.1 || before.1) && b.flex_gap.is_some() {
                column_avoid.boundary(p, (pt, pb, kt), laid, out);
            } else if after.1 || before.1 {
                out.pending.push(Pending::Avoid {
                    top: pt,
                    bottom: pb,
                    floor,
                    next: kt,
                });
            } else {
                column_avoid.reset();
            }
        }
        if k.monolith {
            out.solid.push((kt, kb));
        } else if let Some(inner) = &k.inner {
            if b.flex_gap.is_some() {
                let mut local = BlockBreaks::default();
                walk(inner, kt, kb, laid, &mut local);
                out.append_within(local, kt, kb);
            } else {
                walk(inner, kt, kb, laid, out);
            }
        }
        prev = Some((k, kt, kb));
        last = Some(kb);
    }
    last
}
