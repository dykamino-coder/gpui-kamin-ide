//! Заполнение колонок с обходом нарушений break-*-avoid.

use crate::layout::fragment::plan::Plan;
use crate::layout::fragment::plan::{self, Space};
use crate::layout::fragment::types::Kid;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;

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
    /// без единой лишней копии `Kid`: запреты редки, и у остальных арифметика
    /// тождественна прежней.
    pub(crate) fn fill_avoiding(
        kids: &[Kid],
        target_at: &dyn Fn(usize) -> f32,
        space: Space,
    ) -> (usize, f32, Plan) {
        if !kids.iter().any(|k| k.avoid_before || k.avoid_after) {
            return plan::fill_at(kids, target_at, space);
        }
        let mut best = plan::fill_at(kids, target_at, space);
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
        // Недолаз нарушенной границы переживает перенос разрыва выше: в новом
        // плане он не возникнет (Blink SetupRelayoutData повторяет
        // MinimalSpaceShortage прохода, нашедшего ранний разрыв).
        let mut carried = f32::MAX;
        let done = |best: (usize, f32, Plan), carried: f32| (best.0, best.1.min(carried), best.2);
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
        'outer: loop {
            let Some(bad) =
                Self::avoid_violation_where(kids, &best.2, &|i| stuck.contains(&flow_of(i)))
            else {
                return done(best, carried);
            };
            let flow = flow_of(bad);
            let col = best
                .2
                .iter()
                .filter(|f| f.kid == bad - 1)
                .map(|f| f.col)
                .max();
            if let Some(s) =
                col.and_then(|c| Self::avoid_shortage(kids, &best.2, bad, target_at(c)))
            {
                carried = carried.min(s);
            }
            // Лучшая точка разрыва может лежать ВНУТРИ содержимого, которое
            // уже пройдено (css-break-4 §4.3–4.4; Blink `early_break_` в
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
                        n > COORD_EPS
                            && n < pk.h - COORD_EPS
                            && !pk.forced.iter().any(|&f| (f - n).abs() < COORD_EPS)
                    })
                    .collect();
                for at in cands {
                    let mut e2 = early.clone();
                    e2.push((bad - 1, at));
                    let cand = plan::fill_at(&build(&marks, &e2), target_at, space);
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
                Some(n) if prev.is_none_or(|m| n < m) => n,
                _ if flow == usize::MAX => return done(best, carried),
                _ => {
                    stuck.push(flow);
                    continue;
                }
            };
            marks.retain(|m| m.0 != flow);
            marks.push((flow, next));
            best = plan::fill_at(&build(&marks, &early), target_at, space);
        }
    }
}
