//! Запреты разрывов внутри колонок.
// owner: A

use crate::layout::fragment::types::{Frag, Kid};
use crate::layout::multicol::column_stack::ColumnStack;
mod fill_avoiding;

impl ColumnStack {
    /// Первая граница плана, нарушающая правило 1 css-break-4 §4.3: коробка
    /// `i` начинает НЕ ту колонку, где кончилась `i-1`, хотя разрыв между
    /// ними запрещён (`break-before: avoid*` у неё либо `break-after:
    /// avoid*` у предыдущей). Возвращает `i`; `None` — нарушений нет.
    ///
    /// Принудительный разрыв сильнее запрета («at least one of them forces a
    /// break»), поэтому такие пары пропускаются — на этом стоят зелёные
    /// `break-between-avoid-005/006`, `break-after-table-cell`,
    /// `grid-item-fragmentation-032/044`, где рядом с `avoid` написан
    /// `break-*: column`. Пропускается и разрезанная предыдущая коробка:
    /// разрыв всё равно внутри неё.
    pub(crate) fn first_avoid_violation(kids: &[Kid], plan: &[Frag]) -> Option<usize> {
        Self::avoid_violation_where(kids, plan, &|_| false)
    }

    /// `first_avoid_violation`, пропуская границы, для которых `skip` истинно.
    pub(crate) fn avoid_violation_where(
        kids: &[Kid],
        plan: &[Frag],
        skip: &dyn Fn(usize) -> bool,
    ) -> Option<usize> {
        for i in 1..kids.len() {
            if !(kids[i].avoid_before || kids[i - 1].avoid_after) || skip(i) {
                continue;
            }
            // Начало строки flex (`Par`) — не граница с предыдущим ребёнком:
            // строки — параллельные потоки.
            if kids[i].par.group != 0 && kids[i].par.line_start && !kids[i].par.group_start {
                continue;
            }
            if kids[i].force_before || kids[i - 1].force_after {
                continue;
            }
            // Граница класса A между соседями стоит на низу КОРОБКИ, а не
            // на конце её параллельного потока (css-break-3 §3): куски за
            // `k.h` — уже отдельный поток, и колонку границы они не задают.
            // Без отсечки разрезанный ПОТОК выглядел как разрезанная
            // коробка, проверка «предыдущая сама разрезана» глушила
            // нарушение, и отступ не срабатывал вовсе — так терялись
            // `break-between-avoid-013/014` (поток 60 и 40 не влезал в
            // остаток колонки), тогда как `-011` уцелела: там поток 40 в
            // остаток 50 влезал и коробка оставалась целой.
            // Без потока условие тождественно прежнему: кусок пушится,
            // только пока `from < k.h` (`rest = flow - from > 0`), а нулевая
            // коробка проходит вторым слагаемым.
            let box_h = kids[i - 1].h;
            let prev: Vec<usize> = plan
                .iter()
                .filter(|f| f.kid == i - 1 && (f.from < box_h - 0.01 || f.from <= 0.01))
                .map(|f| f.col)
                .collect();
            let cur: Vec<usize> = plan.iter().filter(|f| f.kid == i).map(|f| f.col).collect();
            let (Some(&prev_start), Some(&prev_end), Some(&cur_start)) =
                (prev.iter().min(), prev.iter().max(), cur.iter().min())
            else {
                continue;
            };
            // Разрыва на этой границе нет — правило не нарушено.
            if cur_start == prev_end {
                continue;
            }
            // Предыдущая сама разрезана: разрыв внутри неё, отступать некуда.
            if prev_start != prev_end {
                continue;
            }
            return Some(i);
        }
        None
    }

    /// На сколько поднять высоту СБАЛАНСИРОВАННЫХ колонок, чтобы снять нарушение
    /// правила 1 css-break-3 §4.3 на границе перед `bad`: первый кусок коробки
    /// `bad` (монолит целиком, иначе до её первой точки класса A, без точек —
    /// целиком, как недолаз `fill_at`) обязан встать в колонку, где кончилась
    /// `bad − 1`. Blink делает то же растяжением: разрыв на границе с `avoid`
    /// получает `kBreakAppealViolatingBreakAvoid` (`break_appeal.h:26`,
    /// `fragmentation_utils.cc:266-270`), это взводит `has_violating_break`
    /// (`column_layout_algorithm.cc:994`), и колонки растут на
    /// `minimal_space_shortage` (`:1168-1170`). `None` — растить не на что.
    pub(crate) fn avoid_shortage(
        kids: &[Kid],
        plan: &[Frag],
        bad: usize,
        target: f32,
    ) -> Option<f32> {
        let prev_col = plan
            .iter()
            .filter(|f| f.kid == bad - 1)
            .map(|f| f.col)
            .max()?;
        let end = plan
            .iter()
            .filter(|f| f.col == prev_col)
            .map(|f| f.y + f.h)
            .fold(0.0f32, f32::max);
        let k = &kids[bad];
        let lead = kids[bad - 1].mb.max(k.mt);
        let piece = if k.monolith {
            k.h
        } else {
            k.cuts
                .iter()
                .map(|&(need, _)| need)
                .find(|&n| n > 0.01)
                .unwrap_or(k.h)
        };
        let d = end + lead + piece - target;
        (d > 0.01).then_some(d)
    }

    /// Сколько принудительных разрывов в линии: между соседями
    /// (`force_before`/`force_after`) и внутри коробок (`forced` строго внутри
    /// `0..h`) — та же разметка, что режет прогоны в `runs_guess`. Нужна одной
    /// проверке Blink `column_layout_algorithm.cc:1152`: при `used_column_count_
    /// <= forced_break_count + 1` мягких точек разрыва нет, и растяжение ради
    /// `avoid` ничего не даст (css-multicol-1 §7 «honoring forced breaks»).
    pub(crate) fn forced_breaks(kids: &[Kid]) -> usize {
        let between = (1..kids.len())
            .filter(|&i| kids[i].force_before || kids[i - 1].force_after)
            .count();
        let inside: usize = kids
            .iter()
            .map(|k| {
                k.forced
                    .iter()
                    .filter(|&&f| f > 0.01 && f < k.h - 0.01)
                    .count()
            })
            .sum();
        between + inside
    }

    /// Ближайшая ВЫШЕ разрешённая граница для отступа от нарушения на `bad`:
    /// наибольшее `j` из `1..bad`, где ни `break-before` коробки `j`, ни
    /// `break-after` коробки `j-1` разрыв не запрещают. `None` — разрешённых
    /// границ нет вовсе, и по css-break-4 §4.3 правило 1 снимается («rules 1,
    /// 2 and 4 are dropped in order to find additional breakpoints»): план
    /// остаётся жадным.
    ///
    /// Ради этой проверки патч и переписан. Без неё отступ уводил в третью
    /// колонку двухколоночный `break-between-avoid-002` (ЧЕТЫРЕ коробки с
    /// `break-before: avoid; break-after: avoid` подряд — запрещены ВСЕ
    /// границы, отступать некуда), то есть ронял зелёную пару. А обход
    /// границ подряд, а не одной, берёт `break-between-avoid-014`: там
    /// граница перед третьей коробкой тоже запрещена, и разрыв обязан
    /// уехать сразу на вторую.
    pub(crate) fn retreat_to(kids: &[Kid], bad: usize) -> Option<usize> {
        // В строке flex (`Par`) отступать можно только внутри своей строки и не
        // на её начало: начало строки — начало группы, перенос всей группы в
        // следующую колонку уводил бы и соседние строки (параллельные потоки),
        // `multi-line-column-flex-fragmentation-028`.
        let lo = if kids[bad].par.group != 0 && !kids[bad].par.group_start {
            (0..=bad)
                .rev()
                .find(|&i| kids[i].par.line_start)
                .map_or(1, |i| i + 1)
        } else {
            1
        };
        (lo.max(1)..bad).rev().find(|&j| {
            !kids[j].avoid_before
                && !kids[j - 1].avoid_after
                && !(kids[j].par.group != 0 && kids[j].par.line_start && !kids[j].par.group_start)
                && !((kids[bad].par.group == 0 || kids[bad].par.group_start)
                    && kids[j].par.group != 0
                    && !kids[j].par.group_start)
        })
    }
}
