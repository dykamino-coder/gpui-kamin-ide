//! Приёмники общего обхода длин (`unit_lengths`): стиль, в который шаг пишет, и проверка без
//! записи, которая только отмечает, что писать есть что.
use super::*;

/// Приёмник обхода. Шаг — пара: `pending` по ссылке отвечает, изменит ли запись стиль, `write`
/// пишет. Проверка идёт только по `pending`: ни копий, ни выделений памяти.
pub(super) trait Target {
    fn rewrite(
        &mut self,
        pending: impl FnOnce(&Computed) -> bool,
        write: impl FnOnce(&mut Computed),
    );
}

impl Target for Computed {
    fn rewrite(
        &mut self,
        pending: impl FnOnce(&Computed) -> bool,
        write: impl FnOnce(&mut Computed),
    ) {
        if pending(self) {
            write(self);
            return;
        }
        // Связка ответа с записью: «нечего писать» — запись на копии обязана её не изменить.
        #[cfg(debug_assertions)]
        {
            let mut copy = self.clone();
            write(&mut copy);
            debug_assert!(copy == *self, "a step wrote without announcing it");
        }
    }
}

/// Проверка без записи: хотя бы один шаг обхода изменил бы стиль.
pub(super) struct Probe<'a> {
    pub(super) style: &'a Computed,
    pub(super) pending: bool,
}

impl Target for Probe<'_> {
    fn rewrite(&mut self, pending: impl FnOnce(&Computed) -> bool, _: impl FnOnce(&mut Computed)) {
        self.pending = self.pending || pending(self.style);
    }
}

/// Шаг над набором длин одной группы: группа берётся на запись, только если длина изменилась.
pub(super) fn lengths<T: Target, const N: usize>(
    target: &mut T,
    moves: impl Fn(Len) -> bool,
    fix: impl Fn(&mut Option<Len>),
    get: impl Fn(&Computed) -> [Option<Len>; N],
    set: impl FnOnce(&mut Computed, [Option<Len>; N]),
) {
    target.rewrite(
        |s| get(s).into_iter().flatten().any(&moves),
        |s| {
            let now = get(s);
            let next = now.map(|mut value| {
                fix(&mut value);
                value
            });
            if next != now {
                set(s, next);
            }
        },
    );
}

/// Шаг над четырьмя сторонами.
pub(super) fn sides<T: Target>(
    target: &mut T,
    moves: impl Fn(Len) -> bool,
    fix: impl Fn(&mut Option<Len>),
    get: fn(&Computed) -> &Sides,
    get_mut: fn(&mut Computed) -> &mut Sides,
) {
    lengths(
        target,
        moves,
        fix,
        |s| {
            let side = get(s);
            [side.top, side.right, side.bottom, side.left]
        },
        |s, [top, right, bottom, left]| {
            let side = get_mut(s);
            (side.top, side.right) = (top, right);
            (side.bottom, side.left) = (bottom, left);
        },
    );
}
