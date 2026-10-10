//! Куски ребёнка по колонкам: цикл размещения фрагментов одного ребёнка (fill_pieces).

use super::{cut_at, fill_clone_dec, forced_or_fits, place_cut};
use crate::layout::fragment::types::{Frag, Kid};
use crate::layout::multicol::column_stack::ColumnStack;

#[allow(clippy::too_many_arguments)]
pub(super) fn fill_pieces(
    target_at: &dyn Fn(usize) -> f32,
    limit: usize,
    paged: bool,
    col: &mut usize,
    y: &mut f32,
    not_top: usize,
    placed: &mut bool,
    shortage: &mut f32,
    out: &mut Vec<Frag>,
    float_hold: &mut Option<(usize, usize, f32)>,
    kid: usize,
    k: &Kid,
    mut cur: f32,
    mut from: f32,
    mut copy: usize,
    flow: f32,
    rg: crate::layout::fragment::types::RepeatGeom,
) {
    loop {
        let target = target_at(*col);
        let room = target - cur;
        // `box-decoration-break: clone` (css-break-4 §break-decoration):
        // блочное украшение стоит в КАЖДОМ фрагменте, из остатка
        // колонки оно вычитается каждый раз — высота коробки =
        // содержимое + N · украшение (Blink
        // `UpdateBorderPaddingForClonedBoxDecorations`). `from` здесь —
        // по СОДЕРЖИМОМУ, `Frag.h` — готовая высота фрагмента:
        // НЕпоследний тянется до низа колонки (§box-splitting: «its
        // content box extends to fill any remaining fragmentainer
        // extent (leaving room for any margins/borders/padding applied
        // by clone)»), последний — по остатку. Монолит сюда не заходит.
        // ★ Откат 07.09 (v153) был НЕ из-за этой ветки: план верен,
        // красное ушло из 14 пар; остатки и потери дала сборка копии
        // (`target/scout-bdb-2026-09-30.md` §3).
        match fill_clone_dec(
            limit, col, y, not_top, placed, shortage, out, float_hold, kid, k, &mut cur, &mut from,
            &mut copy, target, room,
        ) {
            Some(LoopStep::Break) => break,
            Some(LoopStep::Continue) => continue,
            None => {}
        }
        let rest = flow - from;
        // Повтор секций таблицы: фрагмент-продолжение начат ПОД полосой
        // шапки (курсор уже стоит под ней, `RepeatGeom::head_at` при переходе),
        // а непоследний фрагмент оставляет снизу место под подвал —
        // Blink кладёт его сразу за содержимым фрагмента и вычитает из
        // доступного места заранее (`table_layout_algorithm.cc:1145-1149`,
        // `:1323-1327` `reserved_space`). Целиком влезающий остаток
        // подвал несёт сам — он последний в таблице.
        let hd = if copy > 0 { rg.head_at(from) } else { 0.0 };
        let room_all = room;
        let rf = rg.foot_for(from, room_all);
        let room = room_all - rf;
        // Место под содержимым непоследнего фрагмента: секция тянется до
        // низа фрагментаинера (css-break-3 §box-splitting: «its content
        // box extends to fill any remaining fragmentainer extent»), и
        // подвал встаёт на самый низ — `forced-break-before-repeated-
        // footer-001`: ряд 50 с `break-after: column`, подвал на 80..100.
        let ft = |h: f32| {
            if rf > 0.0 {
                (room_all - h).max(rf)
            } else {
                0.0
            }
        };
        // Принудительный разрыв ВНУТРИ коробки раньше её конца и раньше
        // края колонки — режем ровно там.
        let forced = k
            .forced
            .iter()
            .copied()
            .find(|&f| f > from + 0.01 && f < flow - 0.01 && f - from <= room + 0.01);
        match forced_or_fits(
            limit, col, y, not_top, placed, out, float_hold, kid, k, &mut cur, &mut from,
            &mut copy, rg, target, rest, hd, room_all, ft, forced,
        ) {
            Some(LoopStep::Break) => break,
            Some(LoopStep::Continue) => continue,
            None => {}
        }
        // Срез — ПО КРАЮ колонки (css-break-4 §4: slice — правило, не
        // исключение; Blink `FinishFragmentation`). Класс A нужен
        // лишь когда край попал внутрь монолита-потомка или в рамку:
        // тогда разрыв уходит к началу этого диапазона. Точка класса A
        // ровно на краю даёт усечение поля (`nf`).
        let edge = from + room;
        // `break-inside: avoid` — пожелание (css-break-4 §4.4). Коробка
        // ВЫШЕ целого фрагментаинера цельной быть не может: не с верха
        // страницы она уходит на следующую как монолит (ветка
        // `None if placed`), а с верха рвётся как обычная — по точкам
        // класса A, иначе срезом по краю. Так делает Blink: сначала
        // перенос, и только на пустой странице разрыв внутри
        // (`block-page-break-inside-avoid-7/-15-print`).
        let mono =
            k.monolith && !((paged || k.par.avoid_only) && flow > target + 0.01 && cur <= 0.01);
        // Точка разреза `a` с усечением поля по классу A (`nf`).
        let at = |a: f32| -> (f32, f32) {
            let nf = k
                .cuts
                .iter()
                .find(|&&(need, _)| (need - a).abs() < 0.01)
                .map(|&(_, nf)| nf)
                .unwrap_or(a);
            (a, nf)
        };
        let holds = |a: f32, b: f32| a < edge - 0.01 && edge < b - 0.01;
        // Монолит-ПОТОМОК, начатый на верху колонки, не режется краем, а
        // переполняет колонку: кусок идёт до КОНЦА монолита, продолжение —
        // со следующей колонки (css-break-4 §unforced-breaks: «the UA must
        // not break at the top of the page, i.e. it must place at least some
        // content on each fragmentainer»; Blink `fragmentation_utils.cc`
        // `FinishFragmentation`: «If intrinsic block-size is larger than
        // space left, it means that we have some tall unbreakable child
        // content … this fragment will be allowed to take up more space …
        // to encompass the unbreakable content»). Прежде ветка ниже отдавала
        // `None` при `a <= from`, и ребёнок резался по краю колонки прямо
        // сквозь монолит: `monolithic-overflow-003…005.tentative` (два
        // `contain: size` по 100 в колонках по 60), `tall-line-in-short-
        // fragmentainer-000/001` (строка `inline-block` 100 в колонке 50).
        // «Верх колонки» — пустая колонка, нулевой курсор (перед нами только
        // коробки нулевой высоты: разрыв перед монолитом прогресса не даёт,
        // `tall-line-…-000`) либо ПЕРВЫЙ кусок коробки с заданной высотой,
        // которая сама в остаток влезает: её первое содержимое остаётся в
        // колонке, даже переполняя её (Blink `BoxFragmentBuilder::
        // MustStayInCurrentFragmentainer`; `tall-content-inside-constrained-
        // block-000…002`: коробка 25 в остатке 25, внутри `contain: size` 50).
        let at_top =
            !*placed || cur <= 0.01 || (flow > k.h + 0.01 && from <= 0.01 && k.h <= room + 0.01);
        let overflow_to = if k.overflow_top && !mono && room > 0.01 && at_top {
            k.solid
                .iter()
                .filter(|&&(a, b)| holds(a, b) && a <= from + 0.01)
                .map(|&(_, b)| b)
                .fold(None::<f32>, |m, b| Some(m.map_or(b, |x| x.max(b))))
        } else {
            None
        };
        // Монолит дотянулся до конца ребёнка — ребёнок кончается в этой
        // колонке, переполнив её (как монолит-ребёнок в ветке `None =>`).
        if overflow_to.is_some_and(|b| b >= flow - 0.01) {
            out.push(Frag {
                kid,
                copy,
                col: *col,
                y: cur,
                from,
                h: rest,
                head: hd,
                foot: 0.0,
            });
            *y = cur + rest;
            *placed = true;
            break;
        }
        let cut = cut_at(
            paged,
            *placed,
            k,
            from,
            room,
            edge,
            mono,
            at,
            holds,
            overflow_to,
        );
        // Недолаз: на сколько не хватило колонки до ближайшего
        // разреза (или до конца ребёнка).
        let next = k
            .cuts
            .iter()
            .map(|&(need, _)| need - from)
            .find(|&d| d > room + 0.01)
            .unwrap_or(rest);
        *shortage = shortage.min(next - room);
        match place_cut(
            paged, col, y, not_top, placed, out, float_hold, kid, k, &mut cur, &mut from, copy,
            target, rest, hd, room, ft, mono, cut,
        ) {
            Some(LoopStep::Break) => break,
            Some(LoopStep::Continue) => continue,
            None => {}
        }
        if copy + 1 >= limit {
            // Копий больше нет — остаток за кадром.
            *y = target;
            *placed = true;
            break;
        }
        copy += 1;
        *col += 1;
        cur = rg.head_at(from);
        *placed = *col < not_top;
        ColumnStack::skip_float(float_hold, col, &mut cur, placed);
    }
}

/// Выход из шага цикла, вынесенного в функцию: `break` или `continue` вызывающего цикла.
pub(super) enum LoopStep {
    Break,
    Continue,
}
