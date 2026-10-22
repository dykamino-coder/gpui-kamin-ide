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
    // Конечное имя страницы последнего именованного соседа.
    let mut page_prev: Option<&str> = None;
    for k in &b.kids {
        let Some((kt, kb)) = laid.at(k.id) else {
            // Коробка без записи раскладки: её места нет, границы вокруг неё
            // не строятся.
            prev = None;
            last = None;
            continue;
        };
        let renamed = matches!((page_prev, &k.page), (Some(a), Some((b, _))) if a != b);
        if let Some((_, end)) = &k.page {
            page_prev = Some(end);
        }
        if let Some((p, pt, pb)) = prev.filter(|_| k.adjoins) {
            // Распорка роста перед коробкой (`STRUT_ID`) — содержимое
            // предыдущего фрагмента: граница встаёт под ней.
            let pb = laid.at(k.id ^ super::STRUT_ID).map_or(pb, |s| s.1.max(pb));
            out.cuts.push((pb, kt));
            let (after, before) = (p.edge(true, laid), k.edge(false, laid));
            if after.0 || before.0 || renamed {
                out.forced.push(pb);
            } else if after.1 || before.1 {
                out.pending.push(Pending::Avoid {
                    top: pt,
                    bottom: pb,
                    floor,
                    next: kt,
                });
            }
        }
        if k.monolith {
            out.solid.push((kt, kb));
        } else if let Some(inner) = &k.inner {
            walk(inner, kt, kb, laid, out);
        }
        prev = Some((k, kt, kb));
        last = Some(kb);
    }
    last
}
