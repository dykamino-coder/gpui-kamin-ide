//! Разрез ребёнка в колонке: принудительный разрыв, влезание, место разреза и его выпуск.

use super::pieces::LoopStep;
use crate::layout::fragment::types::{Frag, Kid};
use crate::layout::multicol::column_stack::ColumnStack;
mod at;
pub(super) use at::cut_at;

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
