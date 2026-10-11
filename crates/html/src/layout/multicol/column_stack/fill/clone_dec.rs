//! Ребёнок с box-decoration-break: clone — фрагменты с повторяемой декорацией.

use super::pieces::LoopStep;
use crate::layout::fragment::types::{Frag, Kid};
use crate::layout::multicol::column_stack::ColumnStack;

#[allow(clippy::too_many_arguments)]
pub(super) fn fill_clone_dec(
    limit: usize,
    col: &mut usize,
    y: &mut f32,
    not_top: usize,
    placed: &mut bool,
    shortage: &mut f32,
    out: &mut Vec<Frag>,
    float_hold: &mut Option<(usize, usize, f32)>,
    kid: usize,
    k: &Kid,
    cur: &mut f32,
    from: &mut f32,
    copy: &mut usize,
    target: f32,
    room: f32,
) -> Option<LoopStep> {
    if let Some((dt, db)) = k.clone_dec.filter(|_| !k.monolith) {
        let dec = dt + db;
        let content = (k.h - dec).max(0.0);
        let croom = room - dec;
        let rest = content - *from;
        let edge = *from + croom.max(0.0);
        // Принудительный разрыв внутри содержимого: `k.forced` — в
        // координатах КОРОБКИ, содержимое начинается с `dt`.
        let forced = k
            .forced
            .iter()
            .map(|&f| f - dt)
            .find(|&f| f > *from + 0.01 && f < content - 0.01 && f <= edge + 0.01);
        if forced.is_none() && rest <= croom + 0.01 {
            out.push(Frag {
                kid,
                copy: *copy,
                col: *col,
                y: *cur,
                from: *from,
                h: rest + dec,
                head: 0.0,
                foot: 0.0,
            });
            *y = *cur + rest + dec;
            *placed = true;
            return Some(LoopStep::Break);
        }
        *shortage = shortage.min(rest - croom);
        if croom <= 0.01 && *placed {
            *col += 1;
            *cur = 0.0;
            *placed = *col < not_top;
            ColumnStack::skip_float(float_hold, col, cur, placed);
            return Some(LoopStep::Continue);
        }
        // Пустая колонка, где украшению не хватило места, всё равно
        // съедает 1px содержимого — иначе коробка не продвигается
        // (`multicol-zero-height-003`: «it should expend 1px of its
        // content-box per fragment»).
        // Монолит-потомок (css-break-4 §4.1; `k.solid` — в координатах
        // КОРОБКИ): край внутри него — разрыв ПЕРЕД ним; монолит,
        // начатый ровно с `from`, берётся целиком и переполняет
        // фрагмент (css-break-3 §4.1; `clone-012`).
        let before = k
            .solid
            .iter()
            .map(|&(a, b)| (a - dt, b - dt))
            .filter(|&(a, b)| a > *from + 0.01 && a < edge - 0.01 && edge < b - 0.01)
            .map(|(a, _)| a)
            .reduce(f32::min);
        let whole = k
            .solid
            .iter()
            .map(|&(a, b)| (a - dt, b - dt))
            .filter(|&(a, b)| (a - *from).abs() <= 0.01 && b > edge + 0.01)
            .map(|(_, b)| b.min(content))
            .reduce(f32::max);
        let take = match forced {
            Some(f) => f - *from,
            None if croom <= 0.01 => rest.min(1.0),
            None => before.or(whole).map_or(croom, |p| p - *from),
        };
        if take >= rest - 0.01 {
            out.push(Frag {
                kid,
                copy: *copy,
                col: *col,
                y: *cur,
                from: *from,
                h: rest + dec,
                head: 0.0,
                foot: 0.0,
            });
            *y = *cur + rest + dec;
            *placed = true;
            return Some(LoopStep::Break);
        }
        let fh = if croom <= 0.01 { take + dec } else { room };
        out.push(Frag {
            kid,
            copy: *copy,
            col: *col,
            y: *cur,
            from: *from,
            h: fh,
            head: 0.0,
            foot: 0.0,
        });
        *from += take;
        if *copy + 1 >= limit {
            *y = target;
            *placed = true;
            return Some(LoopStep::Break);
        }
        *copy += 1;
        *col += 1;
        *cur = 0.0;
        *placed = *col < not_top;
        ColumnStack::skip_float(float_hold, col, cur, placed);
        return Some(LoopStep::Continue);
    }
    None
}
