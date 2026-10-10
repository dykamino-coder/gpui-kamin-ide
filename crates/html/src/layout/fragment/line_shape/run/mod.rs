//! Форма строчного прогона: текст строки, обрезка по разрывам, сдвиги.

use crate::dom::Element;
use crate::layout::fragment::{LINE_CX, Shape};
use crate::render::{is_blank, measure_font};
use crate::style::values::value::Len;
use crate::text::text_box::normal_fraction;
mod text;
pub(super) use text::brk_trim;
pub(super) use text::line_text;
pub(super) use text::trim_amount;

/// Мера блока со СТРОЧНЫМ содержимым по строкам (css-break-3 §4.3: разрыв
/// «between line boxes» — законная точка класса B; §4.4 `orphans`/`widows`).
/// Высота — строки × высота строки; точки разреза — границы строк, кроме
/// первых `orphans` и последних `widows`. Blink: `inline_layout_algorithm.cc` +
/// `BreakBeforeChildIfNeeded` для строк (`block_layout_algorithm.cc`
/// `HandleInflow` → `IsBreakInside` по строкам). Только при включённом
/// контексте (`with_lines`) и известной ширине колонки; иначе `None`, и мера
/// идёт прежним путём (сплошной строчный набор — монолит).
pub(crate) fn line_run_shape(c: &Element, top: f32, bot: f32, mt: f32, mb: f32) -> Option<Shape> {
    // Высота в точках — коробка своей высоты, строки внутри неё режутся так
    // же (css-break-3 §4.3); строки ниже её низа — переполнение, точек там нет.
    let fixed_h = match c.style.height {
        None | Some(Len::Auto) => None,
        Some(Len::Px(v)) if v >= 0.0 => Some(if c.style.border_box == Some(true) {
            (v - top - bot).max(0.0)
        } else {
            v
        }),
        _ => return None,
    };
    if c.style.min_height.is_some()
        || c.style.max_height.is_some()
        || c.children.iter().all(is_blank)
    {
        return None;
    }
    // Срез строк на РАЗРЫВАХ колонок: `text-box-trim` САМОГО многоколоночника
    // (первый кадр `with_lines`) режет строки у верха и низа каждой колонки
    // (csswg-drafts#5335, comment-2380160677; `text-box-trim-multicol-001`:
    // в первой колонке 4 строки вместо 3, вторая — от самого верха). Срез у
    // блока-ребёнка на разрывах повторяется только при `box-decoration-break:
    // clone` — каждый его фрагмент целая коробка со своей первой и последней
    // строкой (css-break-4 §break-decoration; `-003`); при `slice` — лишь у
    // первой и последней строки (`-002-ref`: вторая колонка с полулидингом).
    // Сторона решается БЛИЖАЙШЕЙ коробкой со срезом этой стороны
    // (`brk_trim`).
    let (inh, w, opts, brk_start, brk_end) = LINE_CX.with(|l| {
        let g = l.borrow();
        let cx = g.as_ref()?;
        let f = cx.frames.last()?;
        Some((
            f.inh.clone(),
            f.w?,
            cx.opts.clone(),
            brk_trim(&cx.frames, |s| s.text_box_trim_start),
            brk_trim(&cx.frames, |s| s.text_box_trim_end),
        ))
    })?;
    if inh.nowrap == Some(true)
        || inh.keep_spaces == Some(true)
        || inh.preserve_newlines == Some(true)
        || inh.letter_spacing.is_some()
        || !matches!(inh.text_indent, None | Some(Len::Px(0.0)))
        || c.first_line.is_some()
        || c.first_letter.is_some()
    {
        return None;
    }
    let text = line_text(&c.children)?;
    let size = match inh.font_size {
        Some(Len::Px(v)) if v > 0.0 => v,
        _ => return None,
    };
    let lh = match inh.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        None => size * normal_fraction(&inh, &opts),
        _ => return None,
    };
    if lh <= 0.0 {
        return None;
    }
    let font = measure_font(&inh, &opts);
    let lines = crate::text::metrics::line_count(&font, size, &text, w)?.max(1);
    let orphans = inh.orphans.unwrap_or(2).max(1) as usize;
    let widows = inh.widows.unwrap_or(2).max(1) as usize;
    // Свой срез первой/последней строки (`blocks()` кладёт его отрицательным
    // полем у первого и последнего ребёнка — коробка ужимается) и срез у
    // разрыва: строка до разрыва кончается на своей метрике, строка после —
    // с неё начинается (`text-box-trim-multicol-001…012`).
    let tt = trim_amount(&inh, size, lh, true);
    let tb = trim_amount(&inh, size, lh, false);
    let shift = if c.style.text_box_trim_start || c.attr("kamin-host-trim-start").is_some() {
        tt
    } else {
        0.0
    };
    let tail = if c.style.text_box_trim_end || c.attr("kamin-host-trim-end").is_some() {
        tb
    } else {
        0.0
    };
    let content = fixed_h.unwrap_or((lines as f32 * lh - shift - tail).max(0.0));
    let h = top + content + bot;
    let cut_need = if brk_end { tb } else { 0.0 };
    let cut_from = if brk_start { tt } else { 0.0 };
    let mut cuts: Vec<(f32, f32)> = (orphans..=lines.saturating_sub(widows))
        .filter(|k| *k >= 1 && *k < lines && (*k as f32) * lh - shift < content - 0.01)
        .map(|k| {
            let at = top + k as f32 * lh - shift;
            ((at - cut_need).max(top), at + cut_from)
        })
        .collect();
    // Разрыв ПЕРЕД первой строкой (блок целиком уходит в следующую колонку):
    // и там строка у верха колонки срезается (`text-box-trim-multicol-012`:
    // `orphans: 2` уводит все строки во вторую колонку, первая — от её верха).
    // Точка в нуле: кусок нулевой высоты в текущей колонке, продолжение — с
    // метрики первой строки.
    if cut_from > 0.0 && shift <= 0.0 && top <= 0.0 && fixed_h.is_none() {
        cuts.insert(0, (0.0, cut_from));
    }
    // Строка неразрывна (css-break-3 §4.3: разрыв только МЕЖДУ строками), а
    // первые `orphans` и последние `widows` строк — одним куском (§4.4).
    // Монолитные диапазоны — промежутки между законными точками: край колонки
    // внутри диапазона уводит разрыв к его началу (`flow.rs` `fill_at`), а не
    // режет строку пополам (балансу было всё равно, где резать: `multicol-
    // margin-001`, кусок 13.33 из строки 20).
    let mut solid = Vec::new();
    let mut from = 0.0f32;
    for &(need, next) in &cuts {
        solid.push((from, need));
        // Срезанная полоса у разрыва — тоже без разрыва внутри: край колонки в
        // ней уводит разрыв к её началу, то есть ровно в точку (`need`), а не
        // режет по краю (`fill_at`: край вне диапазонов — срез по краю).
        if next > need + 0.01 {
            solid.push((need, next));
        }
        from = next;
    }
    solid.push((from, h));
    Some((h, mt, mb, cuts, Vec::new(), solid))
}
