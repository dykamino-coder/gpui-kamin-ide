//! Форма строчного прогона: текст строки, обрезка по разрывам, сдвиги.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::dom::whitespace::is_blank;
use crate::layout::fragment::Shape;
use crate::style::values::value::Len;
use crate::text::fonts::style_font::measure_font;
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
pub(crate) fn line_run_shape(
    c: &Element,
    top: f32,
    bot: f32,
    mt: f32,
    mb: f32,
    vcx: ValueCx<'_>,
) -> Option<Shape> {
    // Высота в точках — коробка своей высоты, строки внутри неё режутся так
    // же (css-break-3 §4.3); строки ниже её низа — переполнение, точек там нет.
    let fixed_h = match c.style.used().height {
        None | Some(Len::Auto) => None,
        Some(Len::Px(v)) if v >= 0.0 => Some(if c.style.used().border_box == Some(true) {
            (v - top - bot).max(0.0)
        } else {
            v
        }),
        _ => return None,
    };
    if c.style.used().min_height.is_some()
        || c.style.used().max_height.is_some()
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
    let (inh, w, opts, brk_start, brk_end, item) = {
        let cx = vcx.lines?;
        let f = cx.frames.last()?;
        let n = cx.frames.len();
        (
            f.inh.clone(),
            f.w?,
            cx.opts.clone(),
            brk_trim(&cx.frames, |s| s.text_box_trim_start),
            brk_trim(&cx.frames, |s| s.text_box_trim_end),
            n >= 2 && cx.frames[n - 2].items != 0,
        )
    };
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
    // A flex or grid item with no specified size up the chain has the
    // initial `medium` (css-fonts-4 §font-size-prop), as the inline layout
    // resolves it: its lines bound the solid part of the item, and the rest
    // of a specified block size fragments (Blink slices a block's remaining
    // block-size after its content). Block flows keep their own measure
    // path, whose anonymous boxes (fieldset, button, details) and markers
    // this measure does not model.
    let size = match inh.font_size {
        Some(Len::Px(v)) if v > 0.0 => v,
        None if item => opts.base_size(),
        _ => return None,
    };
    let lh = match inh.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        None => crate::text::metrics::normal_px(normal_fraction(&inh, &opts), size, opts.cx.vcx()),
        _ => return None,
    };
    if lh <= 0.0 {
        return None;
    }
    let font = measure_font(&inh, &opts);
    let lines = crate::text::metrics::line_count(&font, size, &text, w, opts.cx.vcx())?.max(1);
    let orphans = inh.orphans.unwrap_or(2).max(1) as usize;
    let widows = inh.widows.unwrap_or(2).max(1) as usize;
    // Свой срез первой/последней строки (`blocks()` кладёт его отрицательным
    // полем у первого и последнего ребёнка — коробка ужимается) и срез у
    // разрыва: строка до разрыва кончается на своей метрике, строка после —
    // с неё начинается (`text-box-trim-multicol-001…012`).
    let tt = trim_amount(&inh, size, lh, true, opts.cx.vcx());
    let tb = trim_amount(&inh, size, lh, false, opts.cx.vcx());
    let shift = if c.style.used().text_box_trim_start || c.marks.host_trim_start {
        tt
    } else {
        0.0
    };
    let tail = if c.style.used().text_box_trim_end || c.marks.host_trim_end {
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
    // внутри диапазона уводит разрыв к его началу (`layout::fragment::plan` `fill_at`), а не
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
    // The last line group ends with its lines: a specified height beyond
    // them is the box's unused block size, which fragments like any empty
    // block space (css-break-3 §4.4: no line boxes there; Blink slices a
    // block's remaining block-size after its content).
    let lines_end = top + (lines as f32 * lh - shift - tail).max(0.0);
    let last_end = if fixed_h.is_some() && lines_end < h {
        lines_end.max(from)
    } else {
        h
    };
    solid.push((from, last_end));
    Some(crate::layout::fragment::Shape {
        height: h,
        margin_top: mt,
        margin_bottom: mb,
        cuts,
        forced: Vec::new(),
        solid,
    })
}
