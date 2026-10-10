//! Заполнение колонок.
// owner: A

use crate::layout::fragment::types::{Frag, Kid, NOT_TOP};
use crate::layout::multicol::column_stack::ColumnStack;
mod pieces;
use pieces::fill_pieces;
mod open;
use open::open_kid;
mod cuts;
use cuts::{cut_at, forced_or_fits, place_cut};
mod clone_dec;
use clone_dec::fill_clone_dec;

impl ColumnStack {
    /// Жадная укладка при данной высоте колонки: сколько колонок вышло,
    /// минимальный недолаз и план кусков. Вертикальные поля соседей
    /// СХЛОПЫВАЮТСЯ (CSS 2.1 §8.3.1) и обнуляются на границе колонки
    /// (css-break §5). Не влезший ребёнок режется по ближайшей снизу
    /// ЗАКОННОЙ точке (класс A); без таких точек — по краю колонки, если
    /// он выше колонки, иначе уходит в следующую целиком; монолит режется
    /// никогда и с верха пустой колонки переполняет её (css-break-3 §4.1).
    /// `paged` — стопка СТРАНИЦ, а не колонок: монолит, переполнивший
    /// страницу, занимает место и на следующих (Blink, crbug 1402540:
    /// содержимое после него продолжается там, где кончилось переполнение,
    /// а не с верха следующей страницы — `monolithic-overflow-001`, текст
    /// «в середине второй страницы»). В колонках правило не действует.
    pub(crate) fn fill(
        kids: &[Kid],
        target: f32,
        limit: usize,
        paged: bool,
    ) -> (usize, f32, Vec<Frag>) {
        Self::fill_at(kids, &|_| target, limit, paged)
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

    pub(crate) fn fill_at(
        kids: &[Kid],
        target_at: &dyn Fn(usize) -> f32,
        limit: usize,
        paged: bool,
    ) -> (usize, f32, Vec<Frag>) {
        let mut col = 0usize;
        let mut y = 0.0f32;
        let mut prev_mb = 0.0f32;
        let mut first = true;
        let not_top = NOT_TOP.with(|c| c.get());
        let mut placed = not_top > 0;
        let mut shortage = f32::MAX;
        let mut out: Vec<Frag> = Vec::with_capacity(kids.len());
        let mut force_next = false;
        // Группа параллельных строк (`Par`): место начала `(col, y, placed)` и
        // самый дальний конец строки `(col, y)`.
        let mut group: Option<((usize, f32, bool), (usize, f32))> = None;
        // Перенесённый флоат (`Par::float`): его колонки `первая..=последняя`
        // и конец в последней — поток, дошедший туда, встаёт под ним.
        let mut float_hold: Option<(usize, usize, f32)> = None;
        for (kid, k) in kids.iter().enumerate() {
            let (snap, lead) = open_kid(
                &mut col,
                &mut y,
                &mut prev_mb,
                &mut first,
                not_top,
                &mut placed,
                &mut force_next,
                &mut group,
                &mut float_hold,
                k,
            );
            let cur = y + lead;
            let from = 0.0f32;
            let copy = 0usize;
            // Параллельный поток (css-break-3 §3 «parallel flows»):
            // содержимое, переполняющее коробку с ЗАДАННОЙ высотой,
            // продолжается в следующем фрагментаинере само по себе, а
            // следующий СОСЕД встаёт сразу под коробкой, в той же колонке
            // (Blink `fragmentation_utils.cc`: «If the block-size is
            // constrained / fixed … we know that we're at the end»). Значит
            // режем по `flow`, а курсор соседа откатываем на конец коробки
            // (ниже, перед `prev_mb = k.mb`). Монолит не трогаем: его
            // переполнение по css-break-3 §4.1 остаётся в своей колонке
            // целиком.
            // У `clone` поток не включается: `from`/`h` его кусков — в других
            // координатах (содержимое / готовый фрагмент), и откат курсора
            // ниже их бы не понял. Неразрезанная `clone`-коробка с потоком
            // рисуется `slice` (`render.rs`, `frag_geom.len() > 1`).
            let flow = if k.over > k.h + 0.01 && !k.monolith && k.clone_dec.is_none() {
                k.over
            } else {
                k.h
            };
            // Полосы повтора таблицы (`Repeat::leads`); у прочих детей нули, и
            // укладка тождественна прежней.
            let rg = k.repeat;
            fill_pieces(
                target_at,
                limit,
                paged,
                &mut col,
                &mut y,
                not_top,
                &mut placed,
                &mut shortage,
                &mut out,
                &mut float_hold,
                kid,
                k,
                cur,
                from,
                copy,
                flow,
                rg,
            );
            // Флоат ушёл целиком в следующую колонку, а в текущей осталось
            // место: следующие коробки продолжают её (`Par::float`).
            if k.par.float && k.par.group == 0 && float_hold.is_none() {
                let mut mine = out.iter().filter(|f| f.kid == kid);
                let f0 = mine.next().copied();
                let fl = mine.next_back().copied().or(f0);
                if let (Some(f0), Some(fl)) = (f0, fl)
                    && f0.col > snap.0
                    && snap.1 < target_at(snap.0) - 0.01
                {
                    float_hold = Some((f0.col, fl.col, fl.y + fl.h));
                    (col, y, placed, prev_mb, first) = snap;
                    continue;
                }
            }
            // Откат курсора на конец КОРОБКИ: параллельный поток уехал
            // дальше, но сосед по css-break-3 §3 продолжается там, где
            // кончилась коробка. Ищем кусок ЭТОГО ЖЕ ребёнка, внутрь
            // которого попал `k.h`; если поток оборвался раньше (кончились
            // копии), курсор остаётся где был.
            if flow > k.h + 0.01
                && let Some(f) = out
                    .iter()
                    .rev()
                    .take_while(|f| f.kid == kid)
                    .find(|f| f.from <= k.h + 0.01 && k.h <= f.from + f.h + 0.01)
            {
                col = f.col;
                y = f.y + (k.h - f.from);
                placed = true;
            }
            prev_mb = k.mb;
            first = false;
            // Конец группы строк: дальше поток идёт с самого дальнего конца
            // строки (контейнер кончается вместе с последним фрагментом своих
            // строк; поля контейнера нулевые).
            if k.par.group != 0
                && k.par.group_end
                && let Some((_, end)) = group.take()
            {
                if (col, y) < end {
                    col = end.0;
                    y = end.1;
                }
                placed = true;
                prev_mb = 0.0;
                // `break-after` последних элементов строк — разрыв ПОСЛЕ
                // контейнера (там же, :1915-1918); его несёт последний элемент.
                force_next = k.force_after;
            }
        }
        // Курсор мог быть откачен назад параллельным потоком: колонок нужно
        // столько, сколько занял самый дальний КУСОК, а не сколько прошёл
        // курсор. Без потока значение тождественно прежнему: курсор всегда
        // не меньше любого `f.col`.
        let last = out.iter().map(|f| f.col).fold(col, usize::max);
        (last + 1, shortage, out)
    }
}
