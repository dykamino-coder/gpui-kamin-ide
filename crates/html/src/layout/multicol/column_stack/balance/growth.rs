//! Рост высоты колонок при балансировке: приросты, фрагменты, оценка прогонов.

use crate::layout::fragment::types::{Kid, Rows, StackAxis};
use crate::layout::multicol::column_stack::ColumnStack;

impl ColumnStack {
    /// Точки роста от вытолкнутых монолитов (Blink `FinishFragmentation`,
    /// `fragmentation_utils.cc:641-656`: непоследний фрагмент коробки =
    /// `space_left`; css-flexbox-1 §fragmentation: «A forced break inside a
    /// flex item effectively increases the size of its contents»). Для
    /// каждого куска плана, у которого есть продолжение и который кончается
    /// ровно в НАЧАЛЕ монолитного диапазона (`fill_at`: `holds` → `at(a)`),
    /// а не на принудительном разрыве, — `(ребёнок, смещение разреза,
    /// недобор до низа колонки)`. Только меры, без ширины: `render.rs`
    /// ставит по ним распорки в копии ДО сборки (`grow_pushed`), после чего
    /// монолит стоит ровно на краю и рост здесь выходит нулевым.
    /// Четвёртое поле — «разрыв принудительный»: там распорка обязана быть
    /// ОТДЕЛЬНОЙ КОРОБКОЙ, а не полем. Точка `forced` в мере стоит ПЕРЕД
    /// схлопнутым полем (`shape_full`: `cuts.push((y, y + lead))`, следом
    /// `forced.push(y)`), и рост поля её не двигает — проба
    /// `target/probe-9g/p-single-line-column-flex-fragmentation-022.html`
    /// осталась красной тем же прямоугольником, а `p2-…` с коробкой-распоркой
    /// дала 0.00.
    pub(crate) fn growths(
        kids: &[Kid],
        count: usize,
        fixed_height: Option<f32>,
        rows: Option<Rows>,
        copies: usize,
    ) -> Vec<(usize, f32, f32, bool)> {
        let probe = ColumnStack {
            children: Vec::new(),
            count: count.max(1),
            gap: 0.0,
            axis: StackAxis::Horizontal,
            row_phase: 0.0,
            fixed_height,
            fill_shrink: false,
            rule: None,
            rule_to: None,
            rows,
            copies: copies.max(1),
            gap_items: None,
            intrinsic: None,
            plan: std::cell::RefCell::new(Vec::new()),
            col_w: std::cell::Cell::new(0.0),
            lines_plan: std::cell::RefCell::new(Vec::new()),
            spans_plan: std::cell::RefCell::new(Vec::new()),
        };
        let (_, lines, plan, _) = probe.balance(kids);
        let count = probe.count;
        let mut out = Vec::new();
        for f in &plan {
            let k = &kids[f.kid];
            // У `clone` `from` — по СОДЕРЖИМОМУ, а `h` уже растянут до низа
            // колонки: `from + h` в координатах `solid`/`forced` смысла не
            // имеет, и распорка роста не нужна — фрагмент и так во всю колонку.
            if k.clone_dec.is_some() {
                continue;
            }
            let end = f.from + f.h;
            // Непоследний фрагмент — есть следующая копия. Либо кусок оборван
            // ПРИНУДИТЕЛЬНЫМ разрывом, когда копии кончились (`fill_at`: «Копий
            // больше нет — остаток за кадром»): содержимое после разрыва уходит в
            // переполняющую колонку, а коробка всё равно занимает остаток своей
            // (css-break-3 §box-splitting: «its content box extends to fill any
            // remaining fragmentainer extent»; Blink
            // `ConsumeRemainingFragmentainerSpace`, `block_layout_algorithm.cc:3129`,
            // вызов при принудительном разрыве `:3212`). Без этого фон обёртки во
            // второй колонке `multicol-fill-balance-041` обрывался на 40 из 100.
            let has_next = plan.iter().any(|g| g.kid == f.kid && g.copy == f.copy + 1);
            let cut_short = end < k.h - 0.01 && k.forced.iter().any(|&x| (x - end).abs() < 0.01);
            if !has_next && !cut_short {
                continue;
            }
            // Принудительный разрыв внутри коробки — такой же НЕпоследний
            // фрагмент, как выталкивание монолита: Blink
            // `fragmentation_utils.cc` `FinishFragmentation` даёт ему
            // `min(desired, space_left)`, то есть остаток фрагментаинера
            // целиком (css-flexbox-1 §pagination: «A forced break inside a
            // flex item effectively increases the size of its contents»).
            // Прежде принудительные разрывы отвергались, и фон коробки
            // обрывался на точке разрыва (`single-line-column-flex-
            // fragmentation-022`: колонка 1 красная 50..100).
            let at_solid = k.solid.iter().any(|&(a, _)| (a - end).abs() < 0.01);
            let at_forced = k.forced.iter().any(|&x| (x - end).abs() < 0.01);
            if !at_solid && !at_forced {
                continue;
            }
            let Some(&(_, line_h)) = lines.get(f.col / count) else {
                continue;
            };
            let grow = line_h - f.y - f.h - k.repeat.foot;
            if grow > 0.01 {
                // Монолит растёт от НАЧАЛА своего диапазона; принудительный
                // разрыв стоит ПЕРЕД полем следующей коробки, и распорку надо
                // ставить перед самой коробкой — её верх это `nf` из `cuts`
                // (то же продолжение, что берёт `fill_at`). Парная запись
                // `cuts` у такой точки есть всегда: `shape_full` кладёт
                // `cuts.push((y, y + lead))` и `forced.push(y)` в одном
                // блоке `if !first`.
                let at = if at_solid {
                    end
                } else {
                    k.cuts
                        .iter()
                        .find(|&&(need, _)| (need - end).abs() < 0.01)
                        .map(|&(_, nf)| nf)
                        .unwrap_or(end)
                };
                out.push((f.kid, at, grow, at_forced && !at_solid));
            }
        }
        out
    }

    /// `box-decoration-break: clone`: геометрия фрагментов КАЖДОГО ребёнка —
    /// по копиям `(съеденное содержимое, высота фрагмента)`. Нужна
    /// `render.rs` ДО сборки копий: фрагмент `clone` строится отдельной
    /// коробкой своей высоты. Щуп тот же, что у `growths`.
    pub(crate) fn frags_of(
        kids: &[Kid],
        count: usize,
        fixed_height: Option<f32>,
        rows: Option<Rows>,
        copies: usize,
    ) -> Vec<Vec<(f32, f32)>> {
        let probe = ColumnStack {
            children: Vec::new(),
            count: count.max(1),
            gap: 0.0,
            axis: StackAxis::Horizontal,
            row_phase: 0.0,
            fixed_height,
            fill_shrink: false,
            rule: None,
            rule_to: None,
            rows,
            copies: copies.max(1),
            gap_items: None,
            intrinsic: None,
            plan: std::cell::RefCell::new(Vec::new()),
            col_w: std::cell::Cell::new(0.0),
            lines_plan: std::cell::RefCell::new(Vec::new()),
            spans_plan: std::cell::RefCell::new(Vec::new()),
        };
        let (_, _, plan, _) = probe.balance(kids);
        let mut out: Vec<Vec<(f32, f32)>> = vec![Vec::new(); kids.len()];
        for f in &plan {
            let v = &mut out[f.kid];
            if v.len() <= f.copy {
                v.resize(f.copy + 1, (0.0, 0.0));
            }
            v[f.copy] = (f.from, f.h);
        }
        out
    }

    /// Начальная высота балансировки — по ПРОГОНАМ содержимого между
    /// принудительными разрывами (Blink `ResolveColumnAutoBlockSizeInternal`,
    /// `column_layout_algorithm.cc:1532`, `ContentRuns`: «A content run starts out
    /// as representing one single column, and we'll add as many additional
    /// implicit breaks as needed into the content runs that are the tallest
    /// ones»). css-multicol-1 §Filling Columns: «minimize variations in column
    /// height, while honoring forced breaks». Прогон рвётся на `force_before`/
    /// `force_after` детей и на внутренних `forced` (продолжение — с `nf` из
    /// `cuts`, поле на разрыве усекается). Поля между детьми не считаются — как в
    /// прежней сумме `k.h`: без принудительных разрывов прогон один, и итог
    /// тождественно прежний `total / count`.
    /// Прежняя сумма принимала план, где у единственного ребёнка кончились копии
    /// (`fill_at`: «Копий больше нет — остаток за кадром»): `multicol-fill-
    /// balance-041` (20 | 40 | 100 в двух колонках) брал 80 вместо 100,
    /// `multicol-fill-auto-004` (10|10|10|10|100 в пяти) — 28 вместо 100.
    pub(crate) fn runs_guess(kids: &[Kid], count: usize) -> f32 {
        let mut runs: Vec<f32> = Vec::new();
        let mut cur = 0.0f32;
        let mut started = false;
        let mut force_next = false;
        for k in kids {
            if (k.force_before || force_next) && started {
                runs.push(cur);
                cur = 0.0;
            }
            force_next = k.force_after;
            started = true;
            let mut from = 0.0f32;
            for &f in k.forced.iter().filter(|&&f| f > 0.01 && f < k.h - 0.01) {
                cur += (f - from).max(0.0);
                runs.push(cur);
                cur = 0.0;
                from = k
                    .cuts
                    .iter()
                    .find(|&&(need, _)| (need - f).abs() < 0.01)
                    .map_or(f, |&(_, nf)| nf);
            }
            cur += (k.h - from).max(0.0);
        }
        runs.push(cur);
        // `DistributeImplicitBreaks`: очередной неявный разрыв — в прогон с самой
        // высокой колонкой на данный момент.
        let mut split = vec![1usize; runs.len()];
        for _ in runs.len()..count.max(1) {
            let i = (0..runs.len())
                .max_by(|&a, &b| {
                    (runs[a] / split[a] as f32).total_cmp(&(runs[b] / split[b] as f32))
                })
                .unwrap_or(0);
            split[i] += 1;
        }
        (0..runs.len())
            .map(|i| runs[i] / split[i] as f32)
            .fold(0.0f32, f32::max)
    }
}
