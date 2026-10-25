//! Планировщик фрагментации: укладка детей-`Kid` по фрагментаинерам (колонки и страницы) в план `Frag`.

use crate::layout::fragment::token;
use crate::layout::fragment::types::Kid;
mod result;
pub(crate) use result::{Completion, Plan};
mod pieces;
use pieces::fill_pieces;
mod open;
use open::open_kid;
mod cuts;
use cuts::{cut_at, forced_or_fits, place_cut};
mod clone_dec;
use clone_dec::fill_clone_dec;

/// Фрагментаинеры плана: пространство модели токенов (`token::Space` — вид,
/// начало фрагментаинеров; Blink `ConstraintSpace`) и предел числа
/// фрагментаинеров (копий ребёнка), которого у модели токенов нет: его несёт
/// только укладка по копиям. Высота каждого фрагментаинера — отдельный
/// аргумент `target_at`: её подбирает балансировка.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Space {
    pub(crate) limit: usize,
    pub(crate) space: token::Space,
}

impl Space {
    /// Колонки многоколоночника, все — с верха.
    pub(crate) fn columns(limit: usize) -> Self {
        Space {
            limit,
            space: token::Space::columns(),
        }
    }

    /// Page areas стопки страниц.
    pub(crate) fn pages(limit: usize) -> Self {
        Space {
            limit,
            space: token::Space::pages(),
        }
    }

    /// Сколько первых фрагментаинеров стоят не с верха: первый ряд
    /// вложенного многоколоночника, начатого ниже верха внешней колонки. Не
    /// влезший с верха такой колонки монолит уходит дальше, а не переполняет
    /// её (Blink: `is_at_fragmentainer_start` ложно — `BreakBeforeChildIfNeeded`,
    /// css-break-3 §4.1 «may be pushed»).
    pub(crate) fn with_not_at_start(self, not_at_start: usize) -> Self {
        Space {
            space: self.space.with_not_at_start(not_at_start),
            ..self
        }
    }
}

/// Жадная укладка при данной высоте колонки: сколько колонок вышло,
/// минимальный недолаз и план кусков. Вертикальные поля соседей
/// СХЛОПЫВАЮТСЯ (CSS 2.1 §8.3.1) и обнуляются на границе колонки
/// (css-break §5). Не влезший ребёнок режется по ближайшей снизу
/// ЗАКОННОЙ точке (класс A); без таких точек — по краю колонки, если
/// он выше колонки, иначе уходит в следующую целиком; монолит режется
/// никогда и с верха пустой колонки переполняет её (css-break-3 §4.1).
/// `space.space.is_paginated()` — стопка СТРАНИЦ, а не колонок: монолит, переполнивший
/// страницу, занимает место и на следующих (Blink, crbug 1402540:
/// содержимое после него продолжается там, где кончилось переполнение,
/// а не с верха следующей страницы — `monolithic-overflow-001`, текст
/// «в середине второй страницы»). В колонках правило не действует.
pub(crate) fn fill(kids: &[Kid], target: f32, space: Space) -> (usize, f32, Plan) {
    fill_at(kids, &|_| target, space)
}

/// То же с высотой ПО КОЛОНКАМ: `target_at(col)`. Нужно рядам —
/// полные ряды стоят в `column-height`, хвост балансируется ниже
/// (`balance_tail`).
/// Поток дошёл до колонок перенесённого флоата (`float_hold`): встать
/// под его концом.
pub(crate) fn skip_float(
    hold: &mut Option<(usize, usize, f32)>,
    col: &mut usize,
    cur: &mut f32,
    placed: &mut bool,
) {
    if let Some((fc, lc, ly)) = *hold
        && *col >= fc
    {
        if *col <= lc {
            *col = lc;
            *cur = cur.max(ly);
            *placed = true;
        }
        *hold = None;
    }
}

/// План с постоянной геометрией содержимого.
pub(crate) fn fill_at(
    kids: &[Kid],
    target_at: &dyn Fn(usize) -> f32,
    space: Space,
) -> (usize, f32, Plan) {
    fill_measured(kids, target_at, space, &mut |_| None)
}

mod measured;
pub(crate) use measured::{MeasureRequest, Measurement, fill_measured, fill_resumed};
pub(super) type Remeasure<'a> = dyn FnMut(MeasureRequest) -> Option<(Kid, f32)> + 'a;
pub(super) type ResumeMeasure<'a> = dyn FnMut(MeasureRequest) -> Measurement + 'a;

#[cfg(test)]
mod resumed_tests;
#[cfg(test)]
mod tests;
