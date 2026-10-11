//! Заполнение колонок с обходом нарушений break-*-avoid.

use crate::layout::fragment::types::{Frag, Kid};
use crate::layout::multicol::column_stack::ColumnStack;

impl ColumnStack {
    /// `fill_at` с соблюдением правила 1 css-break-4 §4.3: пока план рвёт
    /// запрещённую границу, разрыв ПЕРЕНОСИТСЯ на ближайшую разрешённую выше
    /// — коробке там ставится принудительный разрыв перед собой, и укладка
    /// повторяется. Это ручная запись blink-овского `early_break_`
    /// (`block_layout_algorithm.cc:1086`, `fragmentation_utils.cc:1250
    /// UpdateEarlyBreakAtBlockChild`): там алгоритм помнит лучшую точку и
    /// переукладывает поддерево, здесь — плоский повтор по списку.
    ///
    /// Метка ОДНА и только двигается назад (`next < m`), а не копится:
    /// накопленные `force_before` ставились разом на двух соседей и разводили
    /// по колонкам ровно ту пару, которую запрет велит держать вместе
    /// (`break-between-avoid-014`: выходило `A | B` `C+D`, а надо `A | B+C+D`).
    ///
    /// Быстрый выход: если запретов нет ни у кого — ровно прежний `fill_at`,
    /// без единой лишней копии `Kid`. Запреты написаны в 101 паре свода из
    /// 23108, у остальных арифметика тождественна прежней.
    pub(crate) fn fill_avoiding(
        kids: &[Kid],
        target_at: &dyn Fn(usize) -> f32,
        limit: usize,
        paged: bool,
    ) -> (usize, f32, Vec<Frag>) {
        if !kids.iter().any(|k| k.avoid_before || k.avoid_after) {
            return Self::fill_at(kids, target_at, limit, paged);
        }
        let mut best = Self::fill_at(kids, target_at, limit, paged);
        // Метка — ОДНА на поток: у обычной стопки поток один, у строк flex
        // (`Par`) — свой у каждой строки, и отступ в одной строке не трогает
        // соседние (параллельные потоки; `multi-line-column-flex-
        // fragmentation-018`: запреты в трёх строках сразу). Поток ребёнка —
        // индекс начала его строки, у обычного ребёнка — `usize::MAX`.
        let flow_of = |i: usize| -> usize {
            // Начало группы — граница самого контейнера с соседом: общий поток.
            if kids[i].par.group == 0 || kids[i].par.group_start {
                return usize::MAX;
            }
            (0..=i).rev().find(|&j| kids[j].par.line_start).unwrap_or(0)
        };
        let mut marks: Vec<(usize, usize)> = Vec::new();
        let mut stuck: Vec<usize> = Vec::new();
        // Ранние разрывы ВНУТРИ предыдущего ребёнка: `(ребёнок, точка)`.
        let mut early: Vec<(usize, f32)> = Vec::new();
        let mut early_tried: Vec<usize> = Vec::new();
        let build = |marks: &[(usize, usize)], early: &[(usize, f32)]| -> Vec<Kid> {
            let mut work: Vec<Kid> = kids.to_vec();
            for &(_, m) in marks {
                work[m].force_before = true;
            }
            for &(k, at) in early {
                work[k].forced.push(at);
                work[k].forced.sort_by(f32::total_cmp);
            }
            work
        };
        'outer: for _ in 0..kids.len().min(8) {
            let Some(bad) =
                Self::avoid_violation_where(kids, &best.2, &|i| stuck.contains(&flow_of(i)))
            else {
                return best;
            };
            let flow = flow_of(bad);
            // Лучшая точка разрыва может лежать ВНУТРИ содержимого, которое
            // уже пройдено (css-break-4 §4.4: «the UA … must choose the
            // breakpoint with the highest break appeal»; Blink `early_break_` в
            // `block_layout_algorithm.cc:1086`, `fragmentation_utils.cc:1250`):
            // запрет на границе с предыдущим ребёнком снимается разрывом в его
            // последней законной точке, а не переносом его целиком —
            // `break-between-avoid-003`: обёртка из трёх квадратов `avoid`,
            // следом квадрат с `break-before: avoid` в колонке 160 → разрыв
            // между вторым и третьим квадратом. Сначала самая поздняя точка;
            // принимается та, что снимает нарушение на этой границе.
            if flow == usize::MAX && !early_tried.contains(&bad) && !kids[bad - 1].monolith {
                early_tried.push(bad);
                let pk = &kids[bad - 1];
                let cands: Vec<f32> = pk
                    .cuts
                    .iter()
                    .rev()
                    .map(|&(need, _)| need)
                    .filter(|&n| {
                        n > 0.01
                            && n < pk.h - 0.01
                            && !pk.forced.iter().any(|&f| (f - n).abs() < 0.01)
                    })
                    .take(4)
                    .collect();
                for at in cands {
                    let mut e2 = early.clone();
                    e2.push((bad - 1, at));
                    let cand = Self::fill_at(&build(&marks, &e2), target_at, limit, paged);
                    let still = Self::avoid_violation_where(kids, &cand.2, &|i| {
                        stuck.contains(&flow_of(i)) || i < bad
                    });
                    if still != Some(bad) {
                        early = e2;
                        best = cand;
                        continue 'outer;
                    }
                }
            }
            let prev = marks.iter().find(|m| m.0 == flow).map(|m| m.1);
            // Метка только назад — иначе цикл вечен, а план качается. Поток,
            // где отступать некуда, больше не трогается.
            let next = match Self::retreat_to(kids, bad) {
                Some(n) if !prev.is_some_and(|m| n >= m) => n,
                _ if flow == usize::MAX => return best,
                _ => {
                    stuck.push(flow);
                    continue;
                }
            };
            marks.retain(|m| m.0 != flow);
            marks.push((flow, next));
            best = Self::fill_at(&build(&marks, &early), target_at, limit, paged);
        }
        best
    }
}
