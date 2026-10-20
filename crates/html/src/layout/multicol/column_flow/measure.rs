//! ColumnFlow: колонки разделённого потока, их замер и внутренняя ширина.

use gpui::{Font, LineFragment, Pixels, Window, px};

/// Места разрезов на колонки и высота потока при заданной ширине.
#[allow(clippy::too_many_arguments)]
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (08.09, v164, `scout-multicol-2026-09.md` MC-BALANCE-CAP,
/// 18 хунков в `flow.rs`/`float.rs`/`render.rs`): заданная блочная высота
/// многоколоночника как потолок высоты КОЛОНКИ (`cap_height` здесь,
/// `balance_line(kids, count, cap)` в `ColumnStack`), чтобы рождались
/// переполняющие колонки по css-multicol-1 §Overflow. Обещание 4…11. Полный
/// свод против v36: +3 (`multicol-fill-balance-041`,
/// `multicol-gap-decorations-005`, `out-of-flow-in-multicolumn-003/007/082`)
/// / −9 (`column-height-025/026/027`, `multicol-fill-balance-003/030`,
/// `-nested-000`, `multicol-nested-021/031`,
/// `fixed-in-nested-multicol-with-viewport-container` → «красное видно»).
/// Потолок ломает вложенные многоколоночники: внешняя высота режет
/// ВНУТРЕННИЙ, у которого своя балансировка. Возвращать только вместе с
/// MC-NESTED (фрагментация вложенного многоколоночника внешним).
pub(super) fn measure_columns(
    text: &str,
    count: Option<usize>,
    col_w: Option<f32>,
    gap: f32,
    font: &Font,
    font_size: f32,
    line_height: f32,
    fill_height: Option<f32>,
    whole: bool,
    line_breaks: (usize, usize),
    width: Pixels,
    window: &mut Window,
) -> (Vec<usize>, usize, Pixels, usize) {
    // Used column-count по фактической ширине (css-multicol §3.4,
    // ResolveUsedColumnCount): `columns: auto <w>` до замера не решается.
    let avail = f32::from(width);
    let from_width = col_w
        .filter(|w| *w > 0.0)
        .map(|w| (((avail + gap) / (w + gap)).floor().max(1.0)) as usize);
    let count = match (count, from_width) {
        (Some(c), Some(fw)) => c.min(fw),
        (Some(c), None) => c,
        (None, Some(fw)) => fw,
        (None, None) => 1,
    };
    // css-multicol-1 §3.4 (11): «W := max(0, (U + column-gap)/N - column-gap)» —
    // колонка уже кегля законна, содержимое из неё вытекает (§8.1: «visibly
    // overflows and is not clipped to the column box»). Прежний сторож отдавал
    // ОДНУ колонку (`multicol-clip-001`: W = 20 при кегле 20, `-gap-large-001`:
    // W = 0, `multicol-count-computed-003/005`). Держал он другое: при такой
    // ширине переносчик gpui рвёт слово АВАРИЙНО (`line_wrapper.rs`, ветка
    // `last_candidate_ix == 0`), и «bl» считался двумя строками — замер
    // `scout-mctextflow-2026-09.md` §5 D: 2.38 от одних буквенных строк. В узком
    // режиме такие границы ниже отбрасываются.
    let inner = ((avail - gap * (count.saturating_sub(1)) as f32) / count as f32).max(0.0);
    let narrow = inner <= font_size;
    // Монолит (`whole`: `contain: size`, css-contain-2 §containment-size «Size
    // containment boxes are monolithic») по строкам между колонками не режется, а
    // текстовый путь видит только его голый текст. В узкой колонке — прежний
    // сторож «без разрезов»: `contain-size-breaks-001` (5 строк Ahem в колонках по
    // 1em) с границами по пробелам давал «A B | C D | E» — не прямоугольник.
    // Широкие колонки — как прежде.
    if narrow && whole {
        return (Vec::new(), count, px(line_height), 1);
    }
    let mut wrapper = window
        .text_system()
        .line_wrapper(font.clone(), px(font_size));
    // Жёсткие разрывы приходят как символ новой строки: каждый сегмент
    // переносится отдельно, начало сегмента — принудительная граница.
    let mut boundaries: Vec<usize> = Vec::new();
    let mut off = 0usize;
    for (i, seg) in text.split('\n').enumerate() {
        if i > 0 {
            boundaries.push(off);
        }
        boundaries.extend(
            wrapper
                .wrap_line_css(&[LineFragment::text(seg)], px(inner))
                // Узкая колонка: только законные возможности переноса — перед
                // границей пробел (css-text-3 §5, `overflow-wrap: normal`). Аварийный
                // разрыв внутри слова отбрасывается, слово вылезает за край колонки,
                // как в рисунке куска (`blocks()` слово не рвёт). Переносчик после
                // аварийного разрыва продолжает считать ширину с него, и следующая
                // законная граница остаётся на месте: «bl ac» при 20 — границы 1, 3, 4,
                // остаётся 3. Широкие колонки — байт-в-байт прежние.
                .filter(|b| !narrow || seg.as_bytes().get(b.ix.wrapping_sub(1)) == Some(&b' '))
                .map(|b| b.ix + off),
        );
        off += seg.len() + 1;
    }
    let lines = boundaries.len() + 1;
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.09): `column-fill: auto` с заданной высотой —
    // колонки заполняются ПОДРЯД до высоты фрагментатора (css-multicol-1
    // §3.3), то есть `per_col = floor(высота / высота строки)`, а не поровну.
    // Высота протягивалась в `ColumnFlow` из `column_flow` (`render.rs`).
    // Срез css-break+css-multicol (1498 пар, 341 зелёная): 342, приобретено
    // 21, потеряно 20. Патч — `target/column-fill.patch`.
    //
    // Важнее самих чисел совпадение: ровно ТЕ ЖЕ двадцать пар
    // (`overflow-clip-004`, `table-cell-expansion-006`,
    // `flex-container-fragmentation-008/009`, `monolithic-with-overflow`,
    // `out-of-flow-in-multicolumn-120/127`, `overflowing-block-003`,
    // `box-shadow-001`, `become-unfragmented-001`) рушатся и от разреза
    // ребёнка по краю колонки (запись у `ColumnStack` в `flow.rs`) — при том
    // что правки совершенно разные. Значит, они зелены не потому, что мы
    // фрагментируем верно, а потому, что не фрагментируем вовсе, и любой
    // ЧАСТИЧНЫЙ шаг их ломает. Отсюда порядок работ: фрагментацию делать
    // одним куском (высота фрагментатора + разрыв между блочными детьми
    // РЕКУРСИВНО + монолиты), а не по частям; поштучные заходы измеримо
    // упираются в +1.
    // `column-fill: auto` (css-multicol-1 §3.3): колонки заполняются ПОДРЯД
    // до высоты фрагментатора, а не делятся поровну. Пока высота не
    // учитывалась вовсе, и заданная высота коробки не влияла на разрезы:
    // строки распределялись ровно по числу колонок.
    let per_col = match fill_height {
        Some(h) if h >= line_height => ((h / line_height).floor() as usize).max(1),
        _ => lines.div_ceil(count).max(1),
    };
    // Разрыв после `k` строк. `orphans`/`widows` (css-break-3 §4.4): в колонке
    // до разрыва не меньше `orphans` строк блока, после — не меньше `widows`.
    // Строки идут одним блоком, и нарушить можно лишь `widows` у последнего
    // разрыва: его переносят к `lines − widows`, но не ближе `orphans` строк
    // от начала колонки.
    let (orphans, widows) = line_breaks;
    let plain: Vec<usize> = (1..count)
        .map(|i| i * per_col)
        .take_while(|&k| k < lines)
        .collect();
    let mut ks: Vec<usize> = Vec::with_capacity(plain.len());
    let mut s = 0usize;
    // `column-fill: auto` с высотой: строки сверх `count` колонок не копятся в
    // последней, а идут ПЕРЕПОЛНЯЮЩИМИ колонками вбок (css-multicol-1 §8.2
    // «additional column boxes are created in the inline direction»; Blink
    // заводит column box на каждый фрагментаинер; `multicol-height-001`: 24
    // строки по 8 — третья колонка за коробкой, и линейка перед ней).
    let max_cols = if fill_height.is_some() {
        lines.max(count)
    } else {
        count
    };
    for _ in 1..max_cols {
        let mut e = s + per_col;
        if e >= lines {
            break;
        }
        if lines - e < widows {
            // Ближе к `widows`, но не ценой `orphans`: когда обоих не
            // соблюсти, разрыв встаёт сразу после `orphans` строк колонки
            // (`widows-orphans-018`: orphans 3, widows 3 — разрыв между 7 и
            // 8, а не между 6 и 7).
            let alt = lines.saturating_sub(widows).max(s + orphans);
            if alt > s && alt < e {
                e = alt;
            }
        }
        ks.push(e);
        s = e;
    }
    // Баланс делит строки поровну на `count` колонок: перенос разрыва, после
    // которого хвост в последнюю колонку не влезает, ему не годится (высоту
    // колонки он не поднимает) — тогда прежние разрезы.
    if fill_height.is_none() && lines - s > per_col {
        ks = plain;
    }
    let cuts: Vec<usize> = ks
        .iter()
        .filter_map(|&k| boundaries.get(k - 1).copied())
        .collect();
    // Строк в первой колонке — по фактическому первому разрыву (после
    // поправки на `orphans`/`widows`), без разрывов — все строки блока.
    let first = ks.first().copied().unwrap_or(lines);
    (cuts, count, px(per_col as f32 * line_height), first)
}

/// Внутренний размер многоколоночного контейнера с ТЕКСТОВЫМ потоком.
///
/// css-sizing-4 `intrinsic-sizing-notes.bs` §multicol-intrinsic — единственное
/// письменное определение (css-multicol-1 §3.4 прямо отказывается его давать).
/// Порядок действий — как в Blink `ColumnLayoutAlgorithm::ComputeMinMaxSizes`.
/// Вклад содержимого у текста берут те же метрики, что и перенос
/// (`LineWrapper::min_content_width` / `max_content_width`), иначе замер и
/// перенос разойдутся между собой.
#[allow(clippy::too_many_arguments)]
pub(super) fn intrinsic_column_width(
    text: &str,
    count: Option<usize>,
    col_w: Option<f32>,
    gap: f32,
    font: &Font,
    font_size: f32,
    min: bool,
    window: &mut Window,
) -> Pixels {
    let mut wrapper = window
        .text_system()
        .line_wrapper(font.clone(), px(font_size));
    let (mut kid_min, mut kid_max) = (0.0f32, 0.0f32);
    // Жёсткие разрывы приходят переводом строки: каждый сегмент — свой абзац,
    // и вклад даёт самый широкий из них.
    for seg in text.split('\n') {
        kid_min = kid_min.max(f32::from(wrapper.min_content_width(seg)));
        kid_max = kid_max.max(f32::from(wrapper.max_content_width(seg)));
    }
    let n = count.unwrap_or(1).max(1) as f32;
    let gap_extra = gap * (n - 1.0);
    let (mut mn, mut mx) = (kid_min, kid_max);
    match col_w.filter(|w| *w > 0.0) {
        Some(w) => {
            mn = mn.min(w);
            mx = mx.max(w).max(mn);
        }
        None => mn = mn * n + gap_extra,
    }
    mx = mx * n + gap_extra;
    px(if min { mn } else { mx })
}
