//! ColumnFlow: колонки разделённого потока, их замер и внутренняя ширина.

use gpui::{Font, LineFragment, Pixels, Window, px};

/// Места разрезов на колонки и высота потока при заданной ширине.
#[allow(clippy::too_many_arguments)]
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
    outer: Option<&crate::layout::fragment::token::space::sequence::Nested>,
    rows: &mut Vec<super::rows::Row>,
    window: &mut Window,
) -> Result<(Vec<usize>, usize, Pixels, usize), crate::layout::fragment::token::Error> {
    rows.clear();
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
    // 2.38 от одних буквенных строк. В узком
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
        return Ok((Vec::new(), count, px(line_height), 1));
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
    if let Some(outer) = outer {
        let plan = super::rows::partition(
            lines,
            count,
            line_height,
            outer,
            fill_height.is_none(),
            line_breaks,
        )?;
        let first = plan.ends.first().copied().unwrap_or(lines);
        let cuts = plan
            .ends
            .iter()
            .filter_map(|&k| boundaries.get(k - 1).copied())
            .collect();
        *rows = plan.rows;
        return Ok((cuts, count, px(plan.height), first));
    }
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
    Ok((cuts, count, px(per_col as f32 * line_height), first))
}
