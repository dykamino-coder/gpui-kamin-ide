//! Балансировка колонок.
// owner: A

use crate::layout::fragment::types::{Frag, Kid, NotTop, Rows, StackAxis};
use crate::layout::multicol::column_stack::ColumnStack;

impl ColumnStack {
    /// Укладка стопки: её высота, линии колонок `(y, высота)`, план кусков и
    /// спаннеры `(ребёнок, y)`. Без рядов — одна линия, как в css-multicol-1
    /// (прежний `balance`).
    /// ★ ЗАМЕРЕНО И ОТКАЧЕНО (10.09, v205/v206, `scout-fragoof-2026-09d.md`):
    /// оба патча захода по внепоточным в многоколоночнике.
    /// №2 «переполняющие колонки» (число копий не упирается в `column-count`
    /// при `column-fill: auto` с заданной высотой, css-multicol-1 §Overflow):
    /// вместе с №1 срез 826 пар дал **+18/−31**.
    /// №1 «статическая позиция прямого внепоточного ребёнка» (щуп нулевой
    /// записью стопки, коробка заместителем `spot_place` после стопки):
    /// в одиночку **+11/−17**.
    /// Потери у обоих одни и те же и лежат в БАЛАНСИРОВКЕ:
    /// `column-height-002/003/004/021/022`, `multicol-containing-003`,
    /// `multicol-fill-balance-038` уходят в «красное видно»,
    /// `multi-line-*-flex-fragmentation-*` разъезжаются. Нулевая запись стопки
    /// и лишние копии одинаково сбивают план балансировки: она считает
    /// содержимое по числу записей, а внепоточная в них не участвует.
    /// Возвращать только вместе с разделением «переполнение» и
    /// «балансировка» в самой `balance`.
    pub(crate) fn balance(
        &self,
        kids: &[Kid],
    ) -> (f32, Vec<(f32, f32)>, Vec<Frag>, Vec<(usize, f32)>) {
        let count = self.count;
        // Потолок баланса без рядов (`Rows::cap`): баланс как прежде, но не
        // выше высоты коробки; копий — сколько построил `render.rs`, лишние
        // колонки переполняют вбок (`place`: `(col, 0.0)`). Линия — высотой
        // в баланс, не в потолок: по ней `growths` меряет рост.
        if let Some(Rows {
            h: Some(lim),
            cap: true,
            ..
        }) = self.rows
        {
            let (h, plan) = self.balance_line(kids, self.copies, Some(lim.max(1.0)));
            return (h, vec![(0.0, h)], plan, Vec::new());
        }
        let Some(rows) = self.rows else {
            // Предел копий поднимает ТОЛЬКО `column-fill: auto` с заданной
            // высотой: там `balance_line` первой строкой уходит в
            // `fill_avoiding` и высоту не подбирает. У балансировки предел
            // остаётся `count` — её условие выхода `cols <= self.count`
            // (Blink `ResolveColumnAutoBlockSize`) от числа копий зависеть не
            // должно.
            let limit = if self.fixed_height.is_some() {
                self.copies.max(count)
            } else {
                count
            };
            let (h, plan) = self.balance_line(kids, limit, None);
            return (h, vec![(0.0, h)], plan, Vec::new());
        };
        let limit = self.copies;
        let Some(h) = rows.h else {
            // Ряд без потолка: колонки балансируются одной линией, а лишние
            // (от принудительных разрывов) идут рядами ниже; высота ряда — по
            // его содержимому (Blink: `HasRowHeight()` ложь, `OffsetToNextRow`
            // = один `row_gap`; `column-wrap-no-constraints-001/002`).
            let (_, plan) = self.balance_line(kids, limit, None);
            let n = plan.iter().map(|f| f.col / count + 1).max().unwrap_or(1);
            let mut out = Vec::with_capacity(n);
            let mut y = 0.0f32;
            for r in 0..n {
                let rh = plan
                    .iter()
                    .filter(|f| f.col / count == r)
                    .map(|f| f.y + f.h)
                    .fold(0.0f32, f32::max);
                out.push((y, rh));
                y += rh + if r + 1 < n { rows.gap } else { 0.0 };
            }
            return (y, out, plan, Vec::new());
        };
        // Blink `ClampedToValidFragmentainerCapacity`: нулевая колонка всё
        // равно вмещает 1px, иначе укладка не сдвинется с места
        // (`columns: 2 / 0`, `column-height-021…023`).
        let cap = h.max(1.0);
        if !rows.wrap {
            // `nowrap` с заданным `column-height`: одна линия высотой ровно в
            // него, баланс не выше потолка (Blink `ConstrainColumnBlockSize`:
            // «Never become taller than used column-height»), лишние
            // колонки — вбок (§8.2 уровня 1; `column-height-005/030`).
            let target = match self.fixed_height {
                Some(_) => cap,
                None => self.balance_line(kids, limit, Some(cap)).0,
            };
            let (_, _, plan) = Self::fill(kids, target, limit, false);
            return (h, vec![(0.0, h)], plan, Vec::new());
        }
        // Курсор по коробке (Blink `intrinsic_block_size_`): сетка рядов с
        // шагом `h + row-gap`; фаза курсора — смещение в текущем ряду
        // (`OffsetInCurrentRow`), остаток ряда `h − фаза` (отрицателен в
        // зазоре). Дети идут ПРОБЕГАМИ: линии колонок между спаннерами и сами
        // спаннеры (`LayoutChildren` → `LayoutFragmentationContext` /
        // `LayoutSpanner`). Без спаннеров — один пробег, план шага 1.
        let stride = h + rows.gap;
        let phase = |y: f32| if stride > 0.0 { y % stride } else { 0.0 };
        // `OffsetToNextRow`: остаток ряда + зазор; ровно с начала ряда — один
        // зазор (так у Blink; на практике сюда не попадаем).
        let next_row = |y: f32| {
            let p = phase(y);
            if p > 0.01 {
                y - p + stride
            } else {
                y + rows.gap
            }
        };
        // Начало рядов со сдвигом `row_phase`: курсор встаёт на фазу внутри
        // первого ряда, и его остаток — первая линия (`balance_run(first)`);
        // в конце координаты возвращаются к верху коробки.
        let phase0 = if h > 0.0 {
            self.row_phase.min(h - 0.01).max(0.0)
        } else {
            0.0
        };
        let _not_top = NotTop::set(if phase0 > 0.01 { count } else { 0 });
        let mut y = phase0;
        let mut lines: Vec<(f32, f32)> = Vec::new();
        let mut plan: Vec<Frag> = Vec::new();
        let mut spans: Vec<(usize, f32)> = Vec::new();
        let mut i = 0usize;
        while i < kids.len() {
            if kids[i].span {
                // Спаннер (§ch: выше ряда — переполняет в следующий, «crossing
                // any row-gap»). Не с начала ряда и не влезает в остаток —
                // со следующего ряда (`LayoutSpanner`: «Not enough room for
                // the spanner in the current row, and we're not at the
                // beginning of the row. Try at the next row»). Поля спаннера —
                // без схлопывания с соседями (упрощение; в тестах кластера
                // нулевые).
                let k = &kids[i];
                let mut at = y + k.mt;
                let p = phase(at);
                if p > 0.01 && h - p < k.h - 0.01 {
                    at = next_row(at);
                }
                spans.push((i, at));
                y = at + k.h + k.mb;
                i += 1;
                continue;
            }
            let j = kids[i..]
                .iter()
                .position(|k| k.span)
                .map_or(kids.len(), |p| i + p);
            // Первая линия пробега — в остаток текущего ряда; остатка нет
            // (курсор в зазоре после спаннера) — со следующего ряда
            // (`LayoutFragmentationContext`: «if there's no room in the
            // current row (because of a preceding spanner, typically)»;
            // нулевой ряд — исключение, `RowHeight() > LayoutUnit()`).
            let mut p = phase(y);
            if h > 0.0 && h - p <= 0.01 {
                y = next_row(y);
                p = 0.0;
            }
            let first = (h - p).max(1.0);
            let row_start = y - p;
            // Линия перед спаннером балансируется всегда, даже при
            // `column-fill: auto` (Blink `:1070`: «We always have to balance
            // columns preceding a spanner»; css-gaps `multicol-gap-
            // decorations-011`: 120px в трёх колонках по 40, а не 60 + 60).
            let balance_last = self.fixed_height.is_none() || j < kids.len();
            let base = lines.len() * count;
            let (n, frags, tail) = self.balance_run(&kids[i..j], first, cap, balance_last);
            for f in frags {
                plan.push(Frag {
                    kid: f.kid + i,
                    col: f.col + base,
                    ..f
                });
            }
            for l in 0..n {
                let ly = if l == 0 {
                    y
                } else {
                    row_start + l as f32 * stride
                };
                let lh = if l + 1 == n {
                    tail
                } else if l == 0 {
                    first
                } else {
                    cap
                };
                lines.push((ly, lh));
            }
            // Курсор — за последней линией: сбалансированная короче ряда, и
            // следующий спаннер ложится прямо за ней (Blink `:1249`).
            y = lines.last().map_or(y, |&(ly, lh)| ly + lh);
            i = j;
        }
        // Последний ряд занимает всю `column-height`, даже если содержимое
        // короче (§ch: «empty space is left»; Blink `Layout()`: «Use all of
        // column-height on the last row as well» — прибавляется и
        // отрицательный остаток: `columns: 2 / 0` даёт `(n − 1) · gap`, как в
        // шаге 1).
        let p = phase(y);
        if p > 0.01 {
            y += h - p;
        }
        for l in lines.iter_mut() {
            l.0 -= phase0;
        }
        for s in spans.iter_mut() {
            s.1 -= phase0;
        }
        (y - phase0, lines, plan, spans)
    }

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

    /// Одна линия колонок: высота заданная (fill:auto) либо баланс «оценка +
    /// добавка на минимальный недолаз» (blink `ResolveColumnAutoBlockSize`);
    /// `cap` — потолок баланса (`ConstrainColumnBlockSize`).
    pub(crate) fn balance_line(
        &self,
        kids: &[Kid],
        limit: usize,
        cap: Option<f32>,
    ) -> (f32, Vec<Frag>) {
        if let Some(h) = self.fixed_height {
            // Правило 1 css-break-4 §4.3 применяется ТОЛЬКО здесь —
            // `column-fill: auto` с заданной высотой колонки. Балансировку
            // (`ResolveColumnAutoBlockSize` ниже) и пробег рядов
            // (`balance_run`) отступ не трогает НАМЕРЕННО: там лишняя колонка
            // от отступа растит `target` на весь недолаз и МЕНЯЕТ высоту
            // многоколоночника. Считано руками на `balance-break-avoidance-002`
            // (сегодня 0.53): баланс уходит 75 -> 125 при верных 100 — то есть
            // в балансе отступ мало поставить, надо ещё выбрать высоту, а это
            // отдельный шаг с отдельным замером.
            let (_, _, slots) = Self::fill_avoiding(kids, &|_| h, limit, false);
            return (h, slots);
        }
        // Нижнее поле ПОСЛЕДНЕГО ребёнка не примыкает к разрыву — многоколоночник
        // свой контекст форматирования (CSS 2.1 §8.3.1), и поле остаётся в
        // содержимом последней колонки: балансу оно нужно так же, как высота
        // (Blink `column_layout_algorithm.cc` `CalculateBalancedColumnBlockSize`
        // считает `content_block_size` с концевым полем). Без него колонки
        // выходили на поле короче (`multicol-nested-002`: 60 против 80). На
        // разрыве поле по-прежнему усекается — оно только у последнего.
        let tail;
        let kids = match kids.last() {
            Some(k) if k.mb > 0.01 && !k.span && k.par.group == 0 => {
                let mut v = kids.to_vec();
                if let Some(l) = v.last_mut() {
                    l.h += l.mb;
                    l.over = l.over.max(l.h);
                    l.mb = 0.0;
                }
                tail = v;
                &tail[..]
            }
            _ => kids,
        };
        // Оценка — по прогонам между принудительными разрывами (`runs_guess`); без
        // них это прежняя `сумма / count`.
        let guess = Self::runs_guess(kids, self.count);
        // Разрезаемая коробка потолка колонке не задаёт: её высоту держит
        // только сумма. Потолок нужен монолитам — они остаются целыми.
        // Баланс не короче самого длинного неразрывного куска: монолита-ребёнка
        // и монолитного диапазона (`solid`) внутри разрезаемого ребёнка (Blink
        // `column_layout_algorithm.cc` — `tallest_unbreakable_block_size` в
        // `CalculateBalancedColumnBlockSize`). Без этого монолит-потомок выше
        // оценки резался краем колонки (`single-line-row-flex-fragmentation-
        // 037`: `contain: size` 100 при оценке 75). Прежний замер (06.09, v103)
        // терял `multicol-overflow-clip`; на integration-7 она цела.
        // Верхнее поле ПЕРВОГО ребёнка не примыкает к разрыву (начало контекста
        // фрагментации, css-break-3 §5.2 усекает поля только у разрыва) — монолит
        // под ним целиком в первой колонке, и Blink растит баланс на недолаз
        // (`PropagateSpaceShortage` у монолита, не влезшего в колонку): три
        // монолита 60 с `margin-top: 20px` у первого — колонки 80, а не 60.
        let tallest = kids
            .iter()
            .enumerate()
            .map(|(i, k)| {
                if k.monolith {
                    k.h + if i == 0 && k.par.group == 0 {
                        k.mt.max(0.0)
                    } else {
                        0.0
                    }
                } else {
                    k.solid.iter().fold(0.0f32, |m, &(a, b)| m.max(b - a))
                }
            })
            .fold(0.0f32, f32::max);
        let clamp = |t: f32| cap.map_or(t, |c| t.min(c));
        let mut target = clamp(guess.max(tallest).max(1.0));
        for _ in 0..6 {
            let (cols, shortage, slots) = Self::fill(kids, target, limit, false);
            if cols <= self.count {
                // Колонок хватило, но план рвёт запрещённую границу (css-break-3
                // §4.3, правило 1) — растим высоту ровно на недолаз
                // (`avoid_shortage`): Blink выходит из цикла балансировки только
                // без нарушений (`column_layout_algorithm.cc:1145-1147`).
                // `balance-break-avoidance-002` 75 → 100, `-001` 50 → 100.
                //
                // Гейты — те же, что у Blink перед растяжением. (1) Рост идёт
                // через `clamp`, то есть в потолок `cap` = заданная высота коробки
                // (`Rows::cap`, `render.rs`; Blink `ConstrainColumnBlockSize`,
                // `:1172`, `:1785-1788`, `:1812`), и если выше не вышло — план
                // принимается С НАРУШЕНИЕМ (`:1175-1178` «Give up if we cannot
                // get taller columns»; css-break-3 §4.3: «rules 1, 2 and 4 are
                // dropped»). ★ Без этого гейта (замер 30.09, `Rows::cap` ещё не
                // было) `flex-/grid-lanes-container-fragmentation-003/004` —
                // `height: 100px`, 50 + 50 + 300 в четырёх колонках — росли
                // 100 → 400 за коробку. (2) Принудительных разрывов не меньше
                // `count − 1` — растягивать бесполезно (`:1152`, `forced_breaks`).
                if Self::forced_breaks(kids) + 1 < self.count
                    && let Some(d) = Self::first_avoid_violation(kids, &slots)
                        .and_then(|bad| Self::avoid_shortage(kids, &slots, bad, target))
                {
                    let grown = clamp(target + d);
                    if grown > target + 0.01 {
                        target = grown;
                        continue;
                    }
                }
                return (target, Self::avoid_at_cap(kids, target, limit, slots));
            }
            // Недолаза не было ни у одной коробки: `shortage` так и остался
            // сторожевым `f32::MAX`. Значит лишние колонки родились
            // ПРИНУДИТЕЛЬНЫМИ разрывами, и растягивать нечего —
            // css-multicol-1 §7: «minimize variations in column height, while
            // honoring forced breaks»; Blink: `if (used_column_count_ <=
            // forced_break_count + 1) break;`.
            // Прежняя строка спрашивала `shortage.is_finite()`, а `f32::MAX` —
            // КОНЕЧНОЕ число: `target += f32::MAX` уводил высоту колонки в
            // `f32::MAX`, затем в бесконечность (`multicol-fill-balance-002`,
            // «Don't overstretch»).
            // Упёрлись в потолок — выше колонкам нельзя, остаток уходит вбок.
            if shortage >= f32::MAX {
                // Лишние колонки — только от принудительных разрывов: каждая
                // колонка держит свой прогон целиком, и высота линии — самый
                // высокий из них (Blink `ContentRuns::DistributeImplicitBreaks`
                // при числе прогонов больше колонок; css-multicol-1 §7
                // «honoring forced breaks»). Стартовая оценка `total / count`
                // выше любого прогона (`grid-item-fragmentation-039`:
                // `columns: 1`, прогоны 100 + 100 — колонка 200 вместо 100).
                let used = slots.iter().map(|f| f.y + f.h).fold(0.0f32, f32::max);
                if used > 0.01 && used < target - 0.01 {
                    let (cols2, _, slots2) = Self::fill(kids, used, limit, false);
                    if cols2 == cols {
                        return (used, slots2);
                    }
                }
                return (target, slots);
            }
            if cap.is_some_and(|c| target >= c) {
                return (target, Self::avoid_at_cap(kids, target, limit, slots));
            }
            // Как blink: расти ровно на минимально необходимое.
            target = clamp(target + if shortage > 0.0 { shortage } else { 1.0 });
        }
        let (_, _, slots) = Self::fill(kids, target, limit, false);
        (target, slots)
    }

    /// Колонки упёрлись в потолок (`Rows::cap`, заданная высота коробки), а
    /// план рвёт запрещённую границу: выше расти нельзя, но окончательная
    /// укладка в колонки этой высоты — обычная фрагментация со своими ранними
    /// разрывами (css-break-3 §4.3 правило 1; Blink балансирует высоту
    /// `CalculateBalancedColumnBlockSize`, а затем раскладывает содержимое
    /// `LayoutRow` той же `block_layout_algorithm` с `early_break_`,
    /// `fragmentation_utils.cc:1250`). `flex-container-fragmentation-003/004`:
    /// 50 + 50 + 300 при `break-before: avoid` у третьей — разрыв после первой.
    pub(crate) fn avoid_at_cap(
        kids: &[Kid],
        target: f32,
        limit: usize,
        slots: Vec<Frag>,
    ) -> Vec<Frag> {
        if Self::first_avoid_violation(kids, &slots).is_none() {
            return slots;
        }
        Self::fill_avoiding(kids, &|_| target, limit, false).2
    }

    /// Пробег линий колонок между спаннерами: первая линия высотой `first`
    /// (остаток текущего ряда), остальные — `cap`; последняя при
    /// `balance_last` балансируется (Blink балансирует КАЖДУЮ линию и режет
    /// её остатком ряда, так что полные ряды выходят ровно в `column-height`,
    /// а последняя — по содержимому: `column-height-003` — 80px остатка в
    /// двух колонках по 40, а не 50 + 30). Возвращает число линий, план
    /// (колонки от нуля, дети от нуля) и высоту последней линии.
    pub(crate) fn balance_run(
        &self,
        kids: &[Kid],
        first: f32,
        cap: f32,
        balance_last: bool,
    ) -> (usize, Vec<Frag>, f32) {
        let count = self.count;
        let limit = self.copies;
        let line_h = |l: usize| if l == 0 { first } else { cap };
        let (cols, _, plan) = Self::fill_at(kids, &|c| line_h(c / count), limit, false);
        let n = cols.div_ceil(count).max(1);
        let last = line_h(n - 1);
        if !balance_last {
            return (n, plan, last);
        }
        let full = (n - 1) * count;
        let rem: f32 = plan.iter().filter(|f| f.col >= full).map(|f| f.h).sum();
        let tallest = plan
            .iter()
            .filter(|f| f.col >= full && kids[f.kid].monolith)
            .map(|f| f.h)
            .fold(0.0f32, f32::max);
        let mut t = (rem / count as f32).ceil().max(tallest).max(1.0);
        if t >= last {
            return (n, plan, last);
        }
        for _ in 0..6 {
            let at = |c: usize| if c < full { line_h(c / count) } else { t };
            let (cols, shortage, slots) = Self::fill_at(kids, &at, limit, false);
            if cols <= full + count {
                return (n, slots, t);
            }
            if shortage >= f32::MAX {
                break;
            }
            t += if shortage > 0.0 { shortage } else { 1.0 };
            if t >= last {
                break;
            }
        }
        (n, plan, last)
    }
}
