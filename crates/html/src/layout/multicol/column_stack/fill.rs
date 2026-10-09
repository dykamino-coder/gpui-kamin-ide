//! Заполнение колонок.
// owner: A

use crate::layout::fragment::types::{Frag, Kid, NOT_TOP};
use crate::layout::multicol::column_stack::ColumnStack;

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
    pub(crate) fn skip_float(hold: &mut Option<(usize, usize, f32)>, col: &mut usize, cur: &mut f32, placed: &mut bool) {
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
            if k.par.group != 0
                && k.par.line_start
                && !k.par.group_start
                && let Some(((sc, sy, sp), end)) = group.as_mut().map(|g| (g.0, &mut g.1))
            {
                // Следующая строка — с того же места, что и вся группа; конец
                // предыдущей запомнен (столбец важнее высоты).
                if (col, y) > *end {
                    *end = (col, y);
                }
                col = sc;
                y = sy;
                placed = sp;
                prev_mb = 0.0;
                force_next = false;
            }
            // Принудительный разрыв перед коробкой или после предыдущей:
            // новая колонка, если текущая не пуста (css-break-4 §3.1). Разрыв
            // перед ПЕРВЫМ элементом любой строки flex перенесён на сам
            // контейнер (Blink `flex_layout_algorithm.cc:1907-1918`: «Treat all
            // columns as a "row" of columns … propagated to the container»;
            // `split_flex_lines`), и у строки второй раз не действует
            // (`multi-line-column-flex-fragmentation-025`).
            let line_head = k.par.group != 0 && k.par.line_start && !k.par.group_start;
            if (k.force_before && !line_head || force_next) && placed {
                col += 1;
                y = 0.0;
                placed = col < not_top;
                Self::skip_float(&mut float_hold, &mut col, &mut y, &mut placed);
                prev_mb = 0.0;
                first = true;
            }
            if k.par.group != 0 && k.par.group_start {
                // Начало группы — коробка самого контейнера (`split_flex_lines`
                // ставит её первой «строкой»): строки начинаются там, где встал
                // её верх (полей и рамок у контейнера нет, поле предыдущего
                // соседа — сквозь него).
                let gy = if first { y } else { y + prev_mb };
                group = Some(((col, gy, placed), (col, gy)));
                y = gy;
                prev_mb = 0.0;
            }
            force_next = k.force_after;
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): схлопывание пары полей по
            // CSS 2.1 §8.3.1 «max положительных + min отрицательных» вместо
            // голого `max`. Срез 3029 пар (css-break/multicol/поля/флоаты
            // CSS2): +0 приобретений, потеря `multi-line-column-flex-
            // fragmentation-032` (0.00 -> 99.00 — страница разъехалась).
            // Правило верное, но в стопке колонок `prev_mb`/`k.mt` уже несут
            // РЕЗУЛЬТАТ схлопывания уровнем выше, и второе применение
            // вычитает отрицательное поле дважды.
            // Элементы строки flex — без схлопывания (css-flexbox-1 §4.2: «The
            // margins of adjacent flex items do not collapse»); первый — от начала
            // группы со своим полем.
            if (k.par.float || k.par.clears)
                && let Some((_, lc, ly)) = float_hold.take()
            {
                col = lc;
                y = ly;
                placed = true;
                first = false;
                prev_mb = 0.0;
            }
            let snap = (col, y, placed, prev_mb, first);
            let lead = if k.par.group != 0 {
                if k.par.line_start { k.mt } else { prev_mb + k.mt }
            } else if first {
                k.mt
            } else {
                prev_mb.max(k.mt)
            };
            let mut cur = y + lead;
            let mut from = 0.0f32;
            let mut copy = 0usize;
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
            loop {
                let target = target_at(col);
                let room = target - cur;
                // `box-decoration-break: clone` (css-break-4 §break-decoration):
                // блочное украшение стоит в КАЖДОМ фрагменте, из остатка
                // колонки оно вычитается каждый раз — высота коробки =
                // содержимое + N · украшение (Blink
                // `UpdateBorderPaddingForClonedBoxDecorations`). `from` здесь —
                // по СОДЕРЖИМОМУ, `Frag.h` — готовая высота фрагмента:
                // НЕпоследний тянется до низа колонки (§box-splitting: «its
                // content box extends to fill any remaining fragmentainer
                // extent (leaving room for any margins/borders/padding applied
                // by clone)»), последний — по остатку. Монолит сюда не заходит.
                // ★ Откат 07.09 (v153) был НЕ из-за этой ветки: план верен,
                // красное ушло из 14 пар; остатки и потери дала сборка копии
                // (`target/scout-bdb-2026-09-30.md` §3).
                if let Some((dt, db)) = k.clone_dec.filter(|_| !k.monolith) {
                    let dec = dt + db;
                    let content = (k.h - dec).max(0.0);
                    let croom = room - dec;
                    let rest = content - from;
                    let edge = from + croom.max(0.0);
                    // Принудительный разрыв внутри содержимого: `k.forced` — в
                    // координатах КОРОБКИ, содержимое начинается с `dt`.
                    let forced = k
                        .forced
                        .iter()
                        .map(|&f| f - dt)
                        .find(|&f| f > from + 0.01 && f < content - 0.01 && f <= edge + 0.01);
                    if forced.is_none() && rest <= croom + 0.01 {
                        out.push(Frag { kid, copy, col, y: cur, from, h: rest + dec, head: 0.0, foot: 0.0 });
                        y = cur + rest + dec;
                        placed = true;
                        break;
                    }
                    shortage = shortage.min(rest - croom);
                    if croom <= 0.01 && placed {
                        col += 1;
                        cur = 0.0;
                        placed = col < not_top;
                        Self::skip_float(&mut float_hold, &mut col, &mut cur, &mut placed);
                        continue;
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
                        .filter(|&(a, b)| a > from + 0.01 && a < edge - 0.01 && edge < b - 0.01)
                        .map(|(a, _)| a)
                        .reduce(f32::min);
                    let whole = k
                        .solid
                        .iter()
                        .map(|&(a, b)| (a - dt, b - dt))
                        .filter(|&(a, b)| (a - from).abs() <= 0.01 && b > edge + 0.01)
                        .map(|(_, b)| b.min(content))
                        .reduce(f32::max);
                    let take = match forced {
                        Some(f) => f - from,
                        None if croom <= 0.01 => rest.min(1.0),
                        None => before.or(whole).map_or(croom, |p| p - from),
                    };
                    if take >= rest - 0.01 {
                        out.push(Frag { kid, copy, col, y: cur, from, h: rest + dec, head: 0.0, foot: 0.0 });
                        y = cur + rest + dec;
                        placed = true;
                        break;
                    }
                    let fh = if croom <= 0.01 { take + dec } else { room };
                    out.push(Frag { kid, copy, col, y: cur, from, h: fh, head: 0.0, foot: 0.0 });
                    from += take;
                    if copy + 1 >= limit {
                        y = target;
                        placed = true;
                        break;
                    }
                    copy += 1;
                    col += 1;
                    cur = 0.0;
                    placed = col < not_top;
                    Self::skip_float(&mut float_hold, &mut col, &mut cur, &mut placed);
                    continue;
                }
                let rest = flow - from;
                // Повтор секций таблицы: фрагмент-продолжение начат ПОД полосой
                // шапки (курсор уже стоит под ней, `RepeatGeom::head_at` при переходе),
                // а непоследний фрагмент оставляет снизу место под подвал —
                // Blink кладёт его сразу за содержимым фрагмента и вычитает из
                // доступного места заранее (`table_layout_algorithm.cc:1145-1149`,
                // `:1323-1327` `reserved_space`). Целиком влезающий остаток
                // подвал несёт сам — он последний в таблице.
                let hd = if copy > 0 { rg.head_at(from) } else { 0.0 };
                let room_all = room;
                let rf = rg.foot_for(from, room_all);
                let room = room_all - rf;
                // Место под содержимым непоследнего фрагмента: секция тянется до
                // низа фрагментаинера (css-break-3 §box-splitting: «its content
                // box extends to fill any remaining fragmentainer extent»), и
                // подвал встаёт на самый низ — `forced-break-before-repeated-
                // footer-001`: ряд 50 с `break-after: column`, подвал на 80..100.
                let ft = |h: f32| if rf > 0.0 { (room_all - h).max(rf) } else { 0.0 };
                // Принудительный разрыв ВНУТРИ коробки раньше её конца и раньше
                // края колонки — режем ровно там.
                let forced = k
                    .forced
                    .iter()
                    .copied()
                    .find(|&f| f > from + 0.01 && f < flow - 0.01 && f - from <= room + 0.01);
                if let Some(f) = forced {
                    let nf = k
                        .cuts
                        .iter()
                        .find(|&&(need, _)| (need - f).abs() < 0.01)
                        .map(|&(_, nf)| nf)
                        .unwrap_or(f);
                    out.push(Frag { kid, copy, col, y: cur, from, h: f - from, head: hd, foot: ft(f - from) });
                    from = nf;
                    if copy + 1 >= limit {
                        y = target;
                        placed = true;
                        break;
                    }
                    copy += 1;
                    col += 1;
                    cur = rg.head_at(from);
                    placed = col < not_top;
                    Self::skip_float(&mut float_hold, &mut col, &mut cur, &mut placed);
                    continue;
                }
                if rest <= room_all + 0.01 {
                    out.push(Frag { kid, copy, col, y: cur, from, h: rest, head: hd, foot: 0.0 });
                    y = cur + rest;
                    placed = true;
                    break;
                }
                // Срез — ПО КРАЮ колонки (css-break-4 §4: slice — правило, не
                // исключение; Blink `FinishFragmentation`). Класс A нужен
                // лишь когда край попал внутрь монолита-потомка или в рамку:
                // тогда разрыв уходит к началу этого диапазона. Точка класса A
                // ровно на краю даёт усечение поля (`nf`).
                let edge = from + room;
                // `break-inside: avoid` — пожелание (css-break-4 §4.4). Коробка
                // ВЫШЕ целого фрагментаинера цельной быть не может: не с верха
                // страницы она уходит на следующую как монолит (ветка
                // `None if placed`), а с верха рвётся как обычная — по точкам
                // класса A, иначе срезом по краю. Так делает Blink: сначала
                // перенос, и только на пустой странице разрыв внутри
                // (`block-page-break-inside-avoid-7/-15-print`).
                let mono = k.monolith
                    && !((paged || k.par.avoid_only) && flow > target + 0.01 && cur <= 0.01);
                // Точка разреза `a` с усечением поля по классу A (`nf`).
                let at = |a: f32| -> (f32, f32) {
                    let nf = k
                        .cuts
                        .iter()
                        .find(|&&(need, _)| (need - a).abs() < 0.01)
                        .map(|&(_, nf)| nf)
                        .unwrap_or(a);
                    (a, nf)
                };
                let holds = |a: f32, b: f32| a < edge - 0.01 && edge < b - 0.01;
                // Монолит-ПОТОМОК, начатый на верху колонки, не режется краем, а
                // переполняет колонку: кусок идёт до КОНЦА монолита, продолжение —
                // со следующей колонки (css-break-4 §unforced-breaks: «the UA must
                // not break at the top of the page, i.e. it must place at least some
                // content on each fragmentainer»; Blink `fragmentation_utils.cc`
                // `FinishFragmentation`: «If intrinsic block-size is larger than
                // space left, it means that we have some tall unbreakable child
                // content … this fragment will be allowed to take up more space …
                // to encompass the unbreakable content»). Прежде ветка ниже отдавала
                // `None` при `a <= from`, и ребёнок резался по краю колонки прямо
                // сквозь монолит: `monolithic-overflow-003…005.tentative` (два
                // `contain: size` по 100 в колонках по 60), `tall-line-in-short-
                // fragmentainer-000/001` (строка `inline-block` 100 в колонке 50).
                // «Верх колонки» — пустая колонка, нулевой курсор (перед нами только
                // коробки нулевой высоты: разрыв перед монолитом прогресса не даёт,
                // `tall-line-…-000`) либо ПЕРВЫЙ кусок коробки с заданной высотой,
                // которая сама в остаток влезает: её первое содержимое остаётся в
                // колонке, даже переполняя её (Blink `BoxFragmentBuilder::
                // MustStayInCurrentFragmentainer`; `tall-content-inside-constrained-
                // block-000…002`: коробка 25 в остатке 25, внутри `contain: size` 50).
                let at_top = !placed
                    || cur <= 0.01
                    || (flow > k.h + 0.01 && from <= 0.01 && k.h <= room + 0.01);
                let overflow_to = if k.overflow_top && !mono && room > 0.01 && at_top {
                    k.solid
                        .iter()
                        .filter(|&&(a, b)| holds(a, b) && a <= from + 0.01)
                        .map(|&(_, b)| b)
                        .fold(None::<f32>, |m, b| Some(m.map_or(b, |x| x.max(b))))
                } else {
                    None
                };
                // Монолит дотянулся до конца ребёнка — ребёнок кончается в этой
                // колонке, переполнив её (как монолит-ребёнок в ветке `None =>`).
                if overflow_to.is_some_and(|b| b >= flow - 0.01) {
                    out.push(Frag { kid, copy, col, y: cur, from, h: rest, head: hd, foot: 0.0 });
                    y = cur + rest;
                    placed = true;
                    break;
                }
                let cut = if let Some(b) = overflow_to {
                    Some(at(b))
                } else if mono || room <= 0.01 {
                    None
                } else if paged && k.solid.iter().any(|&(a, b)| holds(a, b)) {
                    // Страницы: край внутри монолитных диапазонов, а они бывают
                    // ВЛОЖЕНЫ (`avoid` ряда/группы объемлет монолиты ячеек,
                    // `table_shape`). Беречь — самый внешний из тех, что
                    // начинаются ниже `from`. Если и внешний уже начат
                    // (`a <= from`), его не сберечь: с непустой страницы —
                    // перенос целиком (`None if placed`), с верха пустой —
                    // ближайшая внутренняя точка, а не срез по краю (Blink
                    // `FinishFragmentation`: срез = `kBreakAppealLastResort`,
                    // `HasEarlyBreak` → `kNeedsEarlierBreak`; css-break-4
                    // §unforced-breaks: «the UA may use the avoids … to weigh
                    // the appropriateness of the new breakpoints»;
                    // `row-page-break-inside-avoid-1`: «3» на третьем листе в
                    // обеих сторонах пары).
                    let outer = k
                        .solid
                        .iter()
                        .filter(|&&(a, b)| holds(a, b))
                        .map(|&(a, _)| a)
                        .fold(f32::MAX, f32::min);
                    if outer > from + 0.01 {
                        Some(at(outer))
                    } else if placed {
                        None
                    } else {
                        k.solid
                            .iter()
                            .filter(|&&(a, b)| holds(a, b) && a > from + 0.01)
                            .map(|&(a, _)| a)
                            .fold(None::<f32>, |m, a| Some(m.map_or(a, |x| x.min(a))))
                            .map(at)
                    }
                } else if let Some(&(a, _)) = k.solid.iter().find(|&&(a, b)| holds(a, b)) {
                    // Колонки — как прежде: первый содержащий диапазон. Его
                    // начало само может лежать ВНУТРИ другого диапазона —
                    // закрытой запретом границы (`shape_full`, `blk_avoid`):
                    // тогда разрыв уходит к началу и того (`break-between-
                    // avoid-007`: край в монолите c, перед c граница с `break-
                    // before: avoid` — разрыв между a и b, а не перед c).
                    let mut a = a;
                    for _ in 0..4 {
                        match k
                            .solid
                            .iter()
                            .find(|&&(s0, s1)| s0 < a - 0.01 && a < s1 - 0.01 && s0 > from + 0.01)
                        {
                            Some(&(s0, _)) => a = s0,
                            None => break,
                        }
                    }
                    if a > from + 0.01 { Some(at(a)) } else { None }
                } else {
                    Some(at(edge))
                };
                // Недолаз: на сколько не хватило колонки до ближайшего
                // разреза (или до конца ребёнка).
                let next = k
                    .cuts
                    .iter()
                    .map(|&(need, _)| need - from)
                    .find(|&d| d > room + 0.01)
                    .unwrap_or(rest);
                shortage = shortage.min(next - room);
                match cut {
                    Some((need, nf)) => {
                        out.push(Frag { kid, copy, col, y: cur, from, h: (need - from).max(0.0), head: hd, foot: ft((need - from).max(0.0)) });
                        from = nf;
                    }
                    None if !mono && k.cuts.is_empty() && rest > target + 0.01 && room > 0.01 => {
                        // Коробка без точек разреза выше колонки — вид
                        // `slice` по краю (css-break-3 §4).
                        out.push(Frag { kid, copy, col, y: cur, from, h: room, head: hd, foot: ft(room) });
                        from += room;
                    }
                    None if paged && placed && cur > target + 0.01 => {
                        // Страницы: предыдущий монолит ушёл НИЖЕ края листа.
                        // Его переполнение занимает место на следующих
                        // страницах — ребёнок продолжает с той страницы и той
                        // высоты, где переполнение кончилось (Blink,
                        // crbug 1402540; `monolithic-overflow-001`: ref режет
                        // блок 150vh на 1 + 0.5 страницы, тест с `contain:size`
                        // обязан поставить текст в ту же середину 2-й страницы).
                        let skip = (cur / target).floor();
                        col += skip as usize;
                        cur -= skip * target;
                        placed = cur > 0.01;
                        continue;
                    }
                    // Перед ним в колонке только коробки нулевой высоты — это
                    // всё ещё её начало, и разрыва ПЕРЕД ребёнком нет (css-break-4
                    // §unforced-breaks: «must place at least some content on each
                    // fragmentainer»; `tall-break-inside-avoid-at-start`: пустой
                    // `div`, затем `break-inside: avoid` 200 в колонке 100). У
                    // страниц — прежнее правило.
                    None if placed && (cur > 0.01 || paged) => {
                        // Из непустой колонки — в следующую целиком; поле на
                        // границе колонки съедается.
                        col += 1;
                        cur = 0.0;
                        placed = col < not_top;
                        Self::skip_float(&mut float_hold, &mut col, &mut cur, &mut placed);
                        continue;
                    }
                    // Одно поле ребёнка (ни куска содержимого) уже ушло за край
                    // пустой колонки: разрыв ложится в поле, и оно на разрыве
                    // усекается (css-break-3 §5.2: «margins adjoining an
                    // unforced break are truncated»), а коробка начинает
                    // следующую колонку с верха (`flex-container-
                    // fragmentation-006`: поле 200 при колонке 100). Поле —
                    // не содержимое, правило «хоть что-то в каждом
                    // фрагментаинере» его не держит.
                    None if !placed && !paged && from <= 0.01 && cur > target + 0.01 => {
                        col += 1;
                        cur = 0.0;
                        placed = col < not_top;
                        Self::skip_float(&mut float_hold, &mut col, &mut cur, &mut placed);
                        continue;
                    }
                    None if !mono && rest > target + 0.01 && room > 0.01 => {
                        out.push(Frag { kid, copy, col, y: cur, from, h: room, head: hd, foot: ft(room) });
                        from += room;
                    }
                    None => {
                        // Монолит с верха пустой колонки: остаётся и
                        // переполняет.
                        out.push(Frag { kid, copy, col, y: cur, from, h: rest, head: hd, foot: 0.0 });
                        y = cur + rest;
                        placed = true;
                        break;
                    }
                }
                if copy + 1 >= limit {
                    // Копий больше нет — остаток за кадром.
                    y = target;
                    placed = true;
                    break;
                }
                copy += 1;
                col += 1;
                cur = rg.head_at(from);
                placed = col < not_top;
                Self::skip_float(&mut float_hold, &mut col, &mut cur, &mut placed);
            }
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
