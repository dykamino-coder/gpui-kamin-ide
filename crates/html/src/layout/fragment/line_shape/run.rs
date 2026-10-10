//! Форма строчного прогона: текст строки, обрезка по разрывам, сдвиги.

use crate::dom::{Element, Node};
use crate::layout::fragment::{LINE_CX, LineFrame, Shape};
use crate::render::{is_blank, measure_font, out_of_flow};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::text_box::normal_fraction;

/// Текст строчного содержимого для меры строк: `<br>` — `\n`, пробелы
/// схлопнуты (css-text-3 §4.1.1). `None` — среди детей есть то, что строку
/// меняет сверх голого текста (атом, свой шрифт, отбивка, внепоточный).
pub(super) fn line_text(nodes: &[Node]) -> Option<String> {
    fn gather(nodes: &[Node], out: &mut String) -> bool {
        for n in nodes {
            match n {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) if e.tag == "br" => out.push('\u{2028}'),
                // Абсолют в строке места не занимает (CSS 2.1 §9.6): строку
                // не меняет, рисуется копией фрагмента от своего содержащего
                // блока (`css-position/multicol/*-in-multicols`).
                Node::Element(e)
                    if matches!(
                        e.style.position,
                        Some(crate::style::computed::Position::Absolute)
                            | Some(crate::style::computed::Position::Fixed)
                    ) => {}
                Node::Element(e) => {
                    let s = &e.style;
                    let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
                    let b = s.borders();
                    if !e.inline
                        || s.display.is_some()
                        || out_of_flow(s)
                        // Относительный сдвиг куска строку не меняет (CSS 2.1
                        // §9.4.3: «after laying out … shifted»), рисует его копия
                        // (`text-box-trim-multicol-002-ref`: `<span
                        // style="position: relative">`).
                        || !matches!(s.position, None | Some(crate::style::computed::Position::Relative))
                        || s.font_size.is_some()
                        || s.font_family.is_some()
                        || s.font_weight.is_some()
                        || s.italic.is_some()
                        || s.line_height.is_some()
                        || s.vertical_align.is_some()
                        || s.letter_spacing.is_some()
                        || !zero(&s.padding.left)
                        || !zero(&s.padding.right)
                        || !zero(&s.margin.left)
                        || !zero(&s.margin.right)
                        || !zero(&b.left)
                        || !zero(&b.right)
                        || matches!(e.tag.as_str(), "img" | "svg" | "input" | "button" | "select" | "textarea" | "ruby" | "canvas" | "video" | "iframe" | "object" | "embed")
                        || !gather(&e.children, out)
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
    let mut raw = String::new();
    if !gather(nodes, &mut raw) {
        return None;
    }
    let mut out = String::new();
    let mut prev_space = false;
    for ch in raw.chars() {
        if ch == '\u{2028}' {
            if out.ends_with(' ') {
                out.pop();
            }
            out.push('\n');
            prev_space = true;
            continue;
        }
        if matches!(ch, ' ' | '\t' | '\n' | '\r') {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    let mut out = out.trim_matches(' ').to_string();
    // Последний `<br>` строки не открывает (CSS 2.1 §9.4.2: перевод строки
    // завершает текущую строчную коробку; пустой хвостовой коробки нет).
    if out.ends_with('\n') {
        out.pop();
    }
    (!out.trim().is_empty()).then_some(out)
}

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

/// Срез строк у разрыва колонки с одной стороны: ближайшая к строке коробка
/// со срезом этой стороны решает — многоколоночник (первый кадр: каждая
/// колонка — его фрагментаинер) или коробка с `box-decoration-break: clone`
/// режут у каждого разрыва, коробка `slice` — только у своих первой/последней
/// строки, и срез многоколоночника под ней не действует
/// (`text-box-trim-multicol-004`: блок `trim-start` под `trim-both` — низ
/// первой колонки срезан, верх второй нет; `-005` — наоборот).
pub(super) fn brk_trim(frames: &[LineFrame], side: impl Fn(&Computed) -> bool) -> bool {
    // Анонимный блок строк — строки самого хоста, его флаг — копия хостового.
    match frames.iter().rposition(|f| side(&f.inh) && !f.anon) {
        Some(0) => true,
        Some(i) => frames[i].inh.bdb_clone,
        None => false,
    }
}

/// Срез `text-box-trim` с одной стороны строки (css-inline-3 §4.2): полулидинг
/// плюс расстояние от подъёма/спуска до метрики края — та же арифметика, что у
/// `blocks()` (`trim_for`).
pub(super) fn trim_amount(s: &Computed, size: f32, lh: f32, start: bool) -> f32 {
    let family = s.font_family.clone().unwrap_or_default();
    let (ascent, descent, cap) = crate::text::metrics::vmetrics_px(&family, size);
    let half = (lh - (ascent + descent)) / 2.0;
    let edge = if start {
        match s.text_box_over {
            crate::style::computed::TextEdge::Cap => ascent - cap,
            crate::style::computed::TextEdge::Ex => {
                ascent - crate::text::metrics::ch_ex_px(&family, size).1
            }
            _ => 0.0,
        }
    } else {
        match s.text_box_under {
            crate::style::computed::TextEdge::Alphabetic => {
                descent + crate::text::fonts::alphabetic_em(&family) * size
            }
            _ => 0.0,
        }
    };
    (half + edge).max(0.0)
}
