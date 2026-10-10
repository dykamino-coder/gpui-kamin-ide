//! Разрез ребёнка в колонке: принудительный разрыв, влезание, место разреза и его выпуск.

use super::pieces::LoopStep;
use crate::layout::fragment::types::{Frag, Kid};
use crate::layout::multicol::column_stack::ColumnStack;

#[allow(clippy::too_many_arguments)]
pub(super) fn forced_or_fits(
    limit: usize,
    col: &mut usize,
    y: &mut f32,
    not_top: usize,
    placed: &mut bool,
    out: &mut Vec<Frag>,
    float_hold: &mut Option<(usize, usize, f32)>,
    kid: usize,
    k: &Kid,
    cur: &mut f32,
    from: &mut f32,
    copy: &mut usize,
    rg: crate::layout::fragment::types::RepeatGeom,
    target: f32,
    rest: f32,
    hd: f32,
    room_all: f32,
    ft: impl Fn(f32) -> f32,
    forced: Option<f32>,
) -> Option<LoopStep> {
    if let Some(f) = forced {
        let nf = k
            .cuts
            .iter()
            .find(|&&(need, _)| (need - f).abs() < 0.01)
            .map(|&(_, nf)| nf)
            .unwrap_or(f);
        out.push(Frag {
            kid,
            copy: *copy,
            col: *col,
            y: *cur,
            from: *from,
            h: f - *from,
            head: hd,
            foot: ft(f - *from),
        });
        *from = nf;
        if *copy + 1 >= limit {
            *y = target;
            *placed = true;
            return Some(LoopStep::Break);
        }
        *copy += 1;
        *col += 1;
        *cur = rg.head_at(*from);
        *placed = *col < not_top;
        ColumnStack::skip_float(float_hold, col, cur, placed);
        return Some(LoopStep::Continue);
    }
    if rest <= room_all + 0.01 {
        out.push(Frag {
            kid,
            copy: *copy,
            col: *col,
            y: *cur,
            from: *from,
            h: rest,
            head: hd,
            foot: 0.0,
        });
        *y = *cur + rest;
        *placed = true;
        return Some(LoopStep::Break);
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn place_cut(
    paged: bool,
    col: &mut usize,
    y: &mut f32,
    not_top: usize,
    placed: &mut bool,
    out: &mut Vec<Frag>,
    float_hold: &mut Option<(usize, usize, f32)>,
    kid: usize,
    k: &Kid,
    cur: &mut f32,
    from: &mut f32,
    copy: usize,
    target: f32,
    rest: f32,
    hd: f32,
    room: f32,
    ft: impl Fn(f32) -> f32,
    mono: bool,
    cut: Option<(f32, f32)>,
) -> Option<LoopStep> {
    match cut {
        Some((need, nf)) => {
            out.push(Frag {
                kid,
                copy,
                col: *col,
                y: *cur,
                from: *from,
                h: (need - *from).max(0.0),
                head: hd,
                foot: ft((need - *from).max(0.0)),
            });
            *from = nf;
        }
        None if !mono && k.cuts.is_empty() && rest > target + 0.01 && room > 0.01 => {
            // Коробка без точек разреза выше колонки — вид
            // `slice` по краю (css-break-3 §4).
            out.push(Frag {
                kid,
                copy,
                col: *col,
                y: *cur,
                from: *from,
                h: room,
                head: hd,
                foot: ft(room),
            });
            *from += room;
        }
        None if paged && *placed && *cur > target + 0.01 => {
            // Страницы: предыдущий монолит ушёл НИЖЕ края листа.
            // Его переполнение занимает место на следующих
            // страницах — ребёнок продолжает с той страницы и той
            // высоты, где переполнение кончилось (Blink,
            // crbug 1402540; `monolithic-overflow-001`: ref режет
            // блок 150vh на 1 + 0.5 страницы, тест с `contain:size`
            // обязан поставить текст в ту же середину 2-й страницы).
            let skip = (*cur / target).floor();
            *col += skip as usize;
            *cur -= skip * target;
            *placed = *cur > 0.01;
            return Some(LoopStep::Continue);
        }
        // Перед ним в колонке только коробки нулевой высоты — это
        // всё ещё её начало, и разрыва ПЕРЕД ребёнком нет (css-break-4
        // §unforced-breaks: «must place at least some content on each
        // fragmentainer»; `tall-break-inside-avoid-at-start`: пустой
        // `div`, затем `break-inside: avoid` 200 в колонке 100). У
        // страниц — прежнее правило.
        None if *placed && (*cur > 0.01 || paged) => {
            // Из непустой колонки — в следующую целиком; поле на
            // границе колонки съедается.
            *col += 1;
            *cur = 0.0;
            *placed = *col < not_top;
            ColumnStack::skip_float(float_hold, col, cur, placed);
            return Some(LoopStep::Continue);
        }
        // Одно поле ребёнка (ни куска содержимого) уже ушло за край
        // пустой колонки: разрыв ложится в поле, и оно на разрыве
        // усекается (css-break-3 §5.2: «margins adjoining an
        // unforced break are truncated»), а коробка начинает
        // следующую колонку с верха (`flex-container-
        // fragmentation-006`: поле 200 при колонке 100). Поле —
        // не содержимое, правило «хоть что-то в каждом
        // фрагментаинере» его не держит.
        None if !*placed && !paged && *from <= 0.01 && *cur > target + 0.01 => {
            *col += 1;
            *cur = 0.0;
            *placed = *col < not_top;
            ColumnStack::skip_float(float_hold, col, cur, placed);
            return Some(LoopStep::Continue);
        }
        None if !mono && rest > target + 0.01 && room > 0.01 => {
            out.push(Frag {
                kid,
                copy,
                col: *col,
                y: *cur,
                from: *from,
                h: room,
                head: hd,
                foot: ft(room),
            });
            *from += room;
        }
        None => {
            // Монолит с верха пустой колонки: остаётся и
            // переполняет.
            out.push(Frag {
                kid,
                copy,
                col: *col,
                y: *cur,
                from: *from,
                h: rest,
                head: hd,
                foot: 0.0,
            });
            *y = *cur + rest;
            *placed = true;
            return Some(LoopStep::Break);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn cut_at(
    paged: bool,
    placed: bool,
    k: &Kid,
    from: f32,
    room: f32,
    edge: f32,
    mono: bool,
    at: impl Fn(f32) -> (f32, f32),
    holds: impl Fn(f32, f32) -> bool,
    overflow_to: Option<f32>,
) -> Option<(f32, f32)> {
    if let Some(b) = overflow_to {
        Some(at(b))
    } else if mono || room <= 0.01 {
        None
    } else if paged && k.solid.iter().any(|&(a, b)| holds(a, b)) {
        // Страницы: край внутри монолитных диапазонов, а они бывают
        // ВЛОЖЕНЫ (`avoid` ряда/группы объемлет монолиты ячеек,
        // `table_shape`). Беречь — самый внешний из тех, что
        // начинаются ниже `from`. Если и внешний уже начат
        // (`a <= from`), его не сберечь: с непустой страницы —
        // перенос целиком (`None if placed`), с верха пустой —
        // ближайшая внутренняя точка, а не срез по краю (Blink
        // `FinishFragmentation`: срез = `kBreakAppealLastResort`,
        // `HasEarlyBreak` → `kNeedsEarlierBreak`; css-break-4
        // §unforced-breaks: «the UA may use the avoids … to weigh
        // the appropriateness of the new breakpoints»;
        // `row-page-break-inside-avoid-1`: «3» на третьем листе в
        // обеих сторонах пары).
        let outer = k
            .solid
            .iter()
            .filter(|&&(a, b)| holds(a, b))
            .map(|&(a, _)| a)
            .fold(f32::MAX, f32::min);
        if outer > from + 0.01 {
            Some(at(outer))
        } else if placed {
            None
        } else {
            k.solid
                .iter()
                .filter(|&&(a, b)| holds(a, b) && a > from + 0.01)
                .map(|&(a, _)| a)
                .fold(None::<f32>, |m, a| Some(m.map_or(a, |x| x.min(a))))
                .map(at)
        }
    } else if let Some(&(a, _)) = k.solid.iter().find(|&&(a, b)| holds(a, b)) {
        // Колонки — как прежде: первый содержащий диапазон. Его
        // начало само может лежать ВНУТРИ другого диапазона —
        // закрытой запретом границы (`shape_full`, `blk_avoid`):
        // тогда разрыв уходит к началу и того (`break-between-
        // avoid-007`: край в монолите c, перед c граница с `break-
        // before: avoid` — разрыв между a и b, а не перед c).
        let mut a = a;
        for _ in 0..4 {
            match k
                .solid
                .iter()
                .find(|&&(s0, s1)| s0 < a - 0.01 && a < s1 - 0.01 && s0 > from + 0.01)
            {
                Some(&(s0, _)) => a = s0,
                None => break,
            }
        }
        if a > from + 0.01 { Some(at(a)) } else { None }
    } else {
        Some(at(edge))
    }
}
