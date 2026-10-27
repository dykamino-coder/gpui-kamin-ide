//! Разрез ребёнка в колонке: принудительный разрыв, влезание, место разреза и его выпуск.

use super::pieces::LoopStep;
use crate::layout::fragment::plan::Plan;
use crate::layout::fragment::plan::skip_float;
use crate::layout::fragment::types::Kid;
mod at;
mod fragment;
mod inline;
#[cfg(test)]
mod tests;
pub(super) use at::cut_at;
use fragment::FragmentAt;
pub(super) use inline::Tokens as InlineTokens;

#[allow(clippy::too_many_arguments)]
pub(super) fn forced_or_fits(
    limit: usize,
    col: &mut usize,
    y: &mut f32,
    not_top: usize,
    placed: &mut bool,
    out: &mut Plan,
    float_hold: &mut Option<(usize, usize, f32)>,
    kid: usize,
    k: &Kid,
    cur: &mut f32,
    from: &mut f32,
    copy: &mut usize,
    next_head: &dyn Fn(usize, f32) -> f32,
    target: f32,
    rest: f32,
    hd: f32,
    room_all: f32,
    ft: impl Fn(f32) -> f32,
    forced: Option<f32>,
) -> Option<LoopStep> {
    let at = FragmentAt::new(kid, *copy, *col, *cur, *from, hd);
    if let Some(f) = forced {
        let nf = k
            .cuts
            .iter()
            .find(|&&(need, _)| (need - f).abs() < 0.01)
            .map(|&(_, nf)| nf)
            .unwrap_or(f);
        at.emit(out, f - *from, ft(f - *from));
        *from = nf;
        if *copy + 1 >= limit {
            out.incomplete();
            *y = target;
            *placed = true;
            return Some(LoopStep::Break);
        }
        *copy += 1;
        *col += 1;
        *cur = next_head(*col, *from);
        *placed = *col < not_top;
        skip_float(float_hold, col, cur, placed);
        return Some(LoopStep::Continue);
    }
    if rest <= room_all + 0.01 {
        at.emit(out, rest, 0.0);
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
    out: &mut Plan,
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
    let at = FragmentAt::new(kid, copy, *col, *cur, *from, hd);
    match cut {
        Some((need, nf)) => {
            at.emit(out, (need - *from).max(0.0), ft((need - *from).max(0.0)));
            *from = nf;
        }
        None if !mono
            && k.cuts.is_empty()
            && rest > target + 0.01
            && room > 0.01
            && !(*placed && *cur > 0.01 && edge_held(k, *from + room)) =>
        {
            // Коробка без точек разреза выше колонки — вид
            // `slice` по краю (css-break-3 §4). Край внутри монолита
            // (верхняя рамка/отбивка, Blink «avoid breaking inside
            // block-start border») из непустой колонки не режет: ребёнок
            // уходит в следующую целиком (ветка ниже).
            at.emit(out, room, ft(room));
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
            next_column(col, cur, placed, not_top, float_hold);
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
            next_column(col, cur, placed, not_top, float_hold);
            return Some(LoopStep::Continue);
        }
        None if !mono && rest > target + 0.01 && room > 0.01 => {
            at.emit(out, room, ft(room));
            *from += room;
        }
        None => {
            // Монолит с верха пустой колонки: остаётся и
            // переполняет.
            at.emit(out, rest, 0.0);
            *y = *cur + rest;
            *placed = true;
            return Some(LoopStep::Break);
        }
    }
    None
}

/// Край колонки `edge` (в координатах ребёнка) лежит строго внутри одного из
/// его монолитных диапазонов.
fn edge_held(k: &Kid, edge: f32) -> bool {
    k.solid
        .iter()
        .any(|&(a, b)| a < edge - 0.01 && edge < b - 0.01)
}

fn next_column(
    col: &mut usize,
    cur: &mut f32,
    placed: &mut bool,
    not_top: usize,
    float_hold: &mut Option<(usize, usize, f32)>,
) {
    *col += 1;
    *cur = 0.0;
    *placed = *col < not_top;
    skip_float(float_hold, col, cur, placed);
}
