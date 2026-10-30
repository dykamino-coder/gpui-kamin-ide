//! Поток за монолитом, переполнившим колонку: следующий сосед начинается у начала
//! следующего фрагментаинера (css-break-3 §4.1 «the UA must not break at the top of
//! the page … may overflow»; Blink резервирует переполнение монолита только при
//! пагинации — `box_fragment_builder.cc` `ReserveSpaceForMonolithicOverflow` под
//! `IsPaginated()`, — в колонках продолжение потока стоит с верха следующей колонки).
//! Фрагмент у нас — срез непрерывной раскладки копии, поэтому сосед подтягивается к
//! краю фрагмента отрицательной распоркой (`grow_before`). Нужна она параллельному
//! потоку (ячейке ряда), чей разрез задают соседние потоки: свой разрез у конца
//! монолита одиночный поток и так получает (`fill_at`, `overflow_to`). Сам монолит
//! рисуется только в фрагментаинере своего начала (допуск узла,
//! `fragment::replay::Routes`).

use crate::layout::fragment::flow::blocks::{BlockBox, Laid, Pushed};
use crate::layout::fragment::push::Growth;
use crate::tolerance::COORD_EPS;

/// Неделимые монолиты стопки и их разложенный блочный диапазон: каждый рисуется
/// целиком в фрагментаинере своего начала. Обход — итеративный.
pub(crate) fn monoliths(blocks: &BlockBox, laid: &Laid) -> Vec<(u64, f32, f32)> {
    let mut out = Vec::new();
    let mut stack = vec![blocks];
    while let Some(b) = stack.pop() {
        for k in &b.kids {
            if let Some(inner) = &k.inner {
                stack.push(inner);
            }
            if k.unbreakable
                && let Some((top, bottom)) = laid.at(k.id)
            {
                out.push((k.id, top, bottom));
            }
        }
        if let Some(t) = &b.table {
            stack.extend(t.flows());
        }
    }
    out
}

/// Подтяжки соседей за неделимыми монолитами, начатыми во фрагменте `[from, end)` и
/// переполнившими его. Обход — итеративный.
pub(crate) fn pulls(blocks: &BlockBox, laid: &Laid, from: f32, end: f32) -> Vec<Growth> {
    let mut out = Vec::new();
    let mut stack = vec![blocks];
    while let Some(b) = stack.pop() {
        for (i, k) in b.kids.iter().enumerate() {
            if let Some(inner) = &k.inner {
                stack.push(inner);
            }
            if !k.unbreakable {
                continue;
            }
            let Some((top, bottom)) = laid.at(k.id) else {
                continue;
            };
            if top < from - COORD_EPS || top >= end - COORD_EPS || bottom <= end + COORD_EPS {
                continue;
            }
            // Строчное содержимое за монолитом (`adjoins` ложно) отдельной коробки не
            // имеет: его подтянуть нечем.
            let Some(next) = b.kids.get(i + 1).filter(|n| n.adjoins) else {
                continue;
            };
            let Some((next_top, _)) = laid.at(next.id) else {
                continue;
            };
            if next_top > end + COORD_EPS {
                out.push(Growth {
                    target: Pushed::Box(next.id),
                    at: next_top,
                    amount: end - next_top,
                    forced: false,
                });
            }
        }
        if let Some(t) = &b.table {
            stack.extend(t.flows());
        }
    }
    out
}
