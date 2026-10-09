//! Формы строк и рамки строк.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::grid_bands::grid_stack;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::fragment::{LINE_CX, LineFrame, Shape, ShapeCx, with_lines};
use crate::layout::multicol::spanner::{has_deep_spanner, multicol_container};
use crate::layout::page::paged::visible_overflow;
use crate::layout::positioned::predicates::carries_abspos;
use crate::layout::table::anon::anon_element;
use crate::render::{RenderOpts, is_blank, measure_font, out_of_flow};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Align, Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

/// Ширина содержимого блока в потоке родителя шириной `pw` (CSS 2.1 §10.3.3:
/// `margin-left + border + padding + width + … = containing block width`).
/// Только обычный блок потока — у прочих ширину решает своя раскладка.
pub(super) fn line_content_w(c: &Element, pw: f32) -> Option<f32> {
    let s = &c.style;
    // Блочный flex-контейнер и сетка в потоке занимают ширину как блок
    // (css-flexbox-1 §9.2 / css-grid-2 §6.1: «block-level … sized as a
    // block»); ширину ИХ детей решает `items_kind`.
    if c.inline
        || !matches!(
            s.display,
            None | Some(Display::Block) | Some(Display::ListItem) | Some(Display::Flex) | Some(Display::Grid)
        )
        || s.webkit_box == Some(true)
        || s.float.unwrap_or(0) != 0
        || !matches!(s.position, None | Some(crate::style::computed::Position::Relative))
        || table_box(c)
        || multicol_container(s)
    {
        return None;
    }
    let px = |l: &Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    // Боковые поля копии фрагмента кладёт обёртка (`side_margin_wrap`): корень
    // `layout_as_root` своих полей не читает, а под обёрткой копия — обычный
    // ребёнок. ★ Прежде (03.10) замер обёртки дал `multicol-nested-002` 0.00 ->
    // 2.67 из-за концевого поля в балансе — теперь оно в `balance_line`.
    let b = s.borders();
    let edges = px(&s.padding.left)? + px(&s.padding.right)? + px(&b.left)? + px(&b.right)?;
    match s.width {
        Some(Len::Px(w)) => Some(if s.border_box == Some(true) { (w - edges).max(0.0) } else { w }),
        None | Some(Len::Auto) => {
            Some((pw - px(&s.margin.left)? - px(&s.margin.right)? - edges).max(0.0))
        }
        _ => None,
    }
}

/// Элемент КОЛОНКИ flex без переноса с главным размером по `flex-basis`
/// (css-flexbox-1 §9.2 шаг 3): `flex-basis: content` — по содержимому, и
/// `height` при этом не действует. Контейнер `height: auto` свободного места не
/// даёт, и гибкость базу не меняет (§9.7). `None` — мера по `height`
/// элемента, как прежде.
pub(super) fn basis_sized(c: &Element, k: &Element) -> Option<Element> {
    use crate::style::computed::FlexDir;
    let s = &c.style;
    if k.inline
        || s.display != Some(Display::Flex)
        || s.webkit_box == Some(true)
        || s.vertical == Some(true)
        || s.flex_wrap == Some(true)
        || !matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse))
        || !matches!(s.height, None | Some(Len::Auto))
    {
        return None;
    }
    let mut kk = k.clone();
    // База в точках здесь не ставится: при `min-height: auto` элемент не
    // меньше своего содержимого (§4.5), а этой меры у нас нет.
    if k.style.basis_content != Some(true) {
        return None;
    }
    kk.style.height = None;
    Some(kk)
}

/// Хвост непоследнего фрагмента обычной коробки (`flow::StackChild::slack`):
/// фрагмент, разорванный внутри коробки, занимает остаток фрагментаинера
/// (css-break-3 §box-splitting «the box … continues to the end of the
/// fragmentainer»; Blink `fragmentation_utils.cc` «Consumed block-size … is
/// always stretched to the fragmentainers»). Художник хвоста красит его одним
/// цветом по ширине копии — это точно, лишь когда у коробки сплошной фон без
/// картинки и скруглений, а видимые боковые рамки того же цвета. Иначе `None`.
pub(crate) fn slack_fill(c: &Element) -> Option<gpui::Hsla> {
    let s = &c.style;
    let bg = s.background?;
    if s.bg_image.is_some()
        || s.webkit_box == Some(true)
        || [&s.radius.tl, &s.radius.tr, &s.radius.br, &s.radius.bl]
            .into_iter()
            .any(|r| !matches!(r, None | Some(Len::Px(0.0))))
        || !visible_overflow(s)
    {
        return None;
    }
    let b = s.borders();
    for (i, w) in [(1usize, &b.right), (3usize, &b.left)] {
        let wide = match w {
            None => false,
            Some(Len::Px(v)) => *v > 0.0,
            Some(_) => true,
        };
        if wide && s.border_colors[i].or(s.border_color).or(s.color) != Some(bg) {
            return None;
        }
    }
    Some(bg.to_hsla())
}

/// Как ширина детей коробки `c` известна мере строк (`LineFrame::items`).
pub(super) fn items_kind(c: &Element) -> u8 {
    use crate::style::computed::FlexDir;
    let s = &c.style;
    match s.display {
        Some(Display::Flex) => {
            let col = matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse));
            let stretch = matches!(s.align_items, None | Some(Align::Stretch));
            if col && stretch && s.vertical != Some(true) { 1 } else { 2 }
        }
        Some(Display::Grid) => {
            if grid_stack(c) && matches!(s.justify_items, None | Some(Align::Stretch)) { 1 } else { 2 }
        }
        _ => 0,
    }
}

/// Текст строчного содержимого для меры строк: `<br>` — `\n`, пробелы
/// схлопнуты (css-text-3 §4.1.1). `None` — среди детей есть то, что строку
/// меняет сверх голого текста (атом, свой шрифт, отбивка, внепоточный).
fn line_text(nodes: &[Node]) -> Option<String> {
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
                        Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
                    ) => {}
                Node::Element(e) => {
                    let s = &e.style;
                    let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
                    let b = s.borders();
                    if !e.inline
                        || s.display.is_some()
                        || out_of_flow(&s)
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
/// первых `orphans` и последних `widows`. Blink: `inline_layout_algorithm.cc`
/// + `BreakBeforeChildIfNeeded` для строк (`block_layout_algorithm.cc`
/// `HandleInflow` → `IsBreakInside` по строкам). Только при включённом
/// контексте (`with_lines`) и известной ширине колонки; иначе `None`, и мера
/// идёт прежним путём (сплошной строчный набор — монолит).
pub(super) fn line_run_shape(c: &Element, top: f32, bot: f32, mt: f32, mb: f32) -> Option<Shape> {
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
fn brk_trim(frames: &[LineFrame], side: impl Fn(&Computed) -> bool) -> bool {
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
fn trim_amount(s: &Computed, size: f32, lh: f32, start: bool) -> f32 {
    let family = s.font_family.clone().unwrap_or_default();
    let (ascent, descent, cap) = crate::text::metrics::vmetrics_px(&family, size);
    let half = (lh - (ascent + descent)) / 2.0;
    let edge = if start {
        match s.text_box_over {
            crate::style::computed::TextEdge::Cap => ascent - cap,
            crate::style::computed::TextEdge::Ex => ascent - crate::text::metrics::ch_ex_px(&family, size).1,
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

/// Сплошной строчный набор (без блочных детей) — монолит в стопке, ПОКА его
/// строки не измерены (`line_run_shape` дала точки разреза).
pub(crate) fn inline_content(k: &Element) -> bool {
    let block_kid = |n: &Node| {
        matches!(n, Node::Element(x)
            if !x.inline || x.style.display == Some(Display::Block))
    };
    k.children.iter().any(|n| !is_blank(n)) && !k.children.iter().any(block_kid)
}

/// Клон поддерева с длинами коробки в ТОЧКАХ: `em`/`ch`/`ex`/`rem` в
/// размерах, полях, отбивках, рамках и вставках разрешены по кеглю своего
/// элемента (тем же `inline::inherit` → `Computed::resolve_em`, что и при
/// отрисовке). `shape_full` читает голый стиль, и `margin-top: 2em` мерился
/// нулём (нестрогий `px_or`), а `height: 4em` — отказом всей стопки, хотя
/// рисунок кладёт их в точках. Наследуемые (`font-size`, `line-height`) не
/// трогаются: число в `line-height` наследуется множителем.
pub(crate) fn resolved_lengths(c: &Element, parent: &Computed) -> Element {
    let m = crate::style::cascade::inherit::inherit(parent, &c.style);
    let mut t = c.clone();
    // Подменяются ТОЛЬКО шрифтовые единицы, разрешённые в точки: прочее
    // (`None`, `auto`, доли, точки) остаётся как было — `inherit` дописывает
    // и умолчания, а на `min-height.is_some()` стоят гейты строк flex
    // (`multi-line-row-flex-fragmentation-018/037`).
    fn fix(own: &mut Option<Len>, res: Option<Len>) {
        if own.is_some_and(|l| !matches!(l, Len::Px(_) | Len::Auto | Len::Pct(_)))
            && matches!(res, Some(Len::Px(_)))
        {
            *own = res;
        }
    }
    fn fix_sides(own: &mut crate::style::computed::Sides, res: &crate::style::computed::Sides) {
        fix(&mut own.top, res.top);
        fix(&mut own.right, res.right);
        fix(&mut own.bottom, res.bottom);
        fix(&mut own.left, res.left);
    }
    fix(&mut t.style.width, m.width);
    fix(&mut t.style.height, m.height);
    fix(&mut t.style.min_width, m.min_width);
    fix(&mut t.style.min_height, m.min_height);
    fix(&mut t.style.max_width, m.max_width);
    fix(&mut t.style.max_height, m.max_height);
    fix_sides(&mut t.style.margin, &m.margin);
    fix_sides(&mut t.style.padding, &m.padding);
    fix_sides(&mut t.style.border_width, &m.border_width);
    fix_sides(&mut t.style.inset, &m.inset);
    t.children = c
        .children
        .iter()
        .map(|n| match n {
            Node::Element(k) => Node::Element(resolved_lengths(k, &m)),
            other => other.clone(),
        })
        .collect();
    t
}

/// Вложенный многоколоночник, который внешняя стопка ведёт РЯДАМИ (`nest_row`):
/// обычный блок с колонками, высотой в точках, без своих рядов, спаннеров,
/// внепоточных и вертикального письма.
pub(crate) fn nested_rows_box(c: &Element) -> bool {
    let s = &c.style;
    multicol_container(s)
        && (s.column_count.is_some_and(|n| n > 1) || s.column_width.is_some())
        && s.column_height.is_none()
        && s.column_wrap.is_none()
        && (matches!(s.height, Some(Len::Px(h)) if h > 0.0)
            || (matches!(s.height, None | Some(Len::Auto))
                && s.min_height.is_none()
                && s.max_height.is_none()))
        && matches!(s.display, None | Some(Display::Block))
        && s.vertical != Some(true)
        && s.float.unwrap_or(0) == 0
        && !has_deep_spanner(c)
        && !carries_abspos(c, 4)
        // Ряды считаются от верха СОДЕРЖИМОГО: блочные рамка и отбивка (и их
        // повтор у `box-decoration-break: clone`) сдвинули бы границы рядов с
        // границ внешних колонок (`box-decoration-break-clone-010`).
        && !s.bdb_clone
        && {
            // Нижние рамка и отбивка у `height: auto` допустимы: они встают
            // после последнего ряда (`multicol-breaking-006`), а у заданной
            // высоты сдвинули бы последний ряд.
            let b = s.borders();
            let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
            let auto = matches!(s.height, None | Some(Len::Auto));
            zero(&s.padding.top)
                && zero(&b.top)
                && (auto || (zero(&s.padding.bottom) && zero(&b.bottom)))
        }
        && c.children.iter().any(|n| !is_blank(n))
        // Внепоточный потомок — содержащий блок и дотяг рядами не выражены
        // (`out-of-flow-in-multicolumn-019`).
        && !oof_descendant(c)
}

fn oof_descendant(e: &Element) -> bool {
    e.children
        .iter()
        .any(|n| matches!(n, Node::Element(k) if out_of_flow(&k.style) || oof_descendant(k)))
}

/// Мера вложенного многоколоночника с `height: auto`, который внешняя стопка
/// ведёт рядами во внешний фрагментаинер `hh` (`nest_row`): блочный размер —
/// полные ряды по `hh` и сбалансированный последний (css-multicol-1 §7.1
/// «only the last fragment is balanced»; Blink `column_layout_algorithm.cc`
/// `LayoutRow` с `ConstrainColumnBlockSize`), плюс нижние рамка и отбивка.
/// Та же укладка (`ColumnStack::measure_rows`) и те же меры детей
/// (`resolved_lengths` + `with_lines`), что у копии через `element()`.
/// Точек разреза нет: внешняя стопка режет коробку краем колонки — по рядам.
pub(crate) fn nested_rows_shape(c: &Element, parent: &Computed, hh: f32, cw: f32, opts: &RenderOpts) -> Option<Shape> {
    let m = inherit(parent, &c.style);
    let w = nested_box_w(c, cw)?;
    let n = match m.column_count {
        Some(n) if n > 1 => n as usize,
        _ => return None,
    };
    if m.column_width.is_some() {
        return None;
    }
    let gap = match m.column_gap {
        Some(Len::Px(v)) => v,
        _ => match m.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        },
    };
    let col_w = ((w + gap) / n as f32 - gap).max(0.0);
    let mut mc = c.clone();
    mc.style.width = Some(Len::Px(w));
    let g = group_inline_runs(&mc).unwrap_or(mc);
    let kids: Vec<crate::layout::fragment::types::Kid> = with_lines(&m, Some(col_w), opts, || {
        g.children
            .iter()
            .filter(|n| !is_blank(n))
            .map(|n| match n {
                // Только строчное содержимое (анонимные блоки строк): ряды по
                // строкам у нас сходятся с Blink, а блочные дети с
                // переполнением своей коробки и монолиты выше ряда ведут себя
                // иначе (`multicol-fill-balance-003/030`, `multicol-nested-026/
                // 031` при блочных детях уходили 0.00 → «красное видно»).
                Node::Element(k)
                    if !k.inline
                        && inline_content(k)
                        && !out_of_flow(&k.style)
                        && matches!(k.style.position, None | Some(crate::style::computed::Position::Relative))
                        && k.style.float.unwrap_or(0) == 0 =>
                {
                    let k = resolved_lengths(k, &m);
                    let sh = shape_full(&k, 4, ShapeCx::COLUMNS)?;
                    Some(crate::layout::fragment::types::Kid {
                        h: sh.0,
                        mt: sh.1,
                        mb: sh.2,
                        monolith: solid_box(&k) && !(inline_content(&k) && !sh.3.is_empty()),
                        cuts: sh.3,
                        force_before: edge_break(&k, false),
                        force_after: edge_break(&k, true),
                        avoid_before: edge_avoid(&k, false),
                        avoid_after: edge_avoid(&k, true),
                        forced: sh.4,
                        solid: sh.5,
                        span: false,
                        over: sh.0,
                        clone_dec: None,
                        overflow_top: false,
                        repeat: Default::default(),
                        par: Default::default(),
                    })
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
    })?;
    if kids.is_empty() {
        return None;
    }
    let fixed = (m.column_fill_auto == Some(true)).then_some(hh);
    let rows = crate::layout::fragment::types::Rows {
        h: Some(hh),
        gap: 0.0,
        wrap: true,
        cap: false,
    };
    let content = crate::layout::multicol::column_stack::ColumnStack::measure_rows(&kids, n, gap, fixed, rows);
    let px = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let b = c.style.borders();
    let bot = px(&c.style.padding.bottom)? + px(&b.bottom)?;
    let h = content + bot;
    let solid = if bot > 0.0 { vec![(content, h)] } else { Vec::new() };
    Some((h, 0.0, px(&c.style.margin.bottom)?, Vec::new(), Vec::new(), solid))
}

/// Ширина коробки (`width` по её `box-sizing`) ребёнка в колонке `cw`.
pub(crate) fn nested_box_w(c: &Element, cw: f32) -> Option<f32> {
    let s = &c.style;
    let px = |l: &Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let b = s.borders();
    let outer = cw - px(&s.margin.left)? - px(&s.margin.right)?;
    let edges = px(&s.padding.left)? + px(&s.padding.right)? + px(&b.left)? + px(&b.right)?;
    Some(if s.border_box == Some(true) { outer } else { outer - edges }.max(0.0))
}

/// Копия ребёнка стопки встаёт КОРНЕМ (`flow.rs` `layout_as_root` во всю
/// колонку), а корень taffy своих полей не кладёт: боковое поле `margin: 0 1em`
/// пропадало, текст ложился от края колонки (`multicol-nested-002`). Обёртка-
/// колонка делает копию обычным ребёнком: её поля и растяжение решает
/// раскладка (CSS 2.1 §10.3.3). Только горизонтальная стопка и только при
/// ненулевых полях в точках — иначе копия прежняя.
pub(crate) fn side_margin_wrap(el: AnyElement, copy: &Element, vertical: bool) -> AnyElement {
    let nz = |l: &Option<Len>| matches!(l, Some(Len::Px(v)) if v.abs() > 0.001);
    if vertical || !(nz(&copy.style.margin.left) || nz(&copy.style.margin.right)) {
        return el;
    }
    div().flex().flex_col().w_full().child(el).into_any_element()
}

/// Строчные прогоны среди блочных детей многоколоночника — в анонимные блоки
/// (CSS 2.1 §9.2.1.1: «If a block container box has a block-level box inside
/// it, then we force it to have only block-level boxes inside it» — строчное
/// содержимое оборачивается анонимной блочной коробкой). Тогда стопка колонок
/// видит их обычными детьми и режет по строкам. `None` — заворачивать нечего.
pub(crate) fn group_inline_runs(e: &Element) -> Option<Element> {
    let inline_level = |n: &Node| match n {
        Node::Text(_) => true,
        Node::Element(k) => k.inline && !out_of_flow(&k.style) && k.style.display.is_none(),
    };
    if !e.children.iter().any(|n| inline_level(n) && !is_blank(n)) {
        return None;
    }
    let mut out: Vec<Node> = Vec::new();
    let mut run: Vec<Node> = Vec::new();
    let flush = |run: &mut Vec<Node>, out: &mut Vec<Node>| {
        if run.iter().all(is_blank) {
            out.append(run);
        } else {
            out.push(Node::Element(anon_element("anon-block", std::mem::take(run))));
        }
    };
    for n in &e.children {
        if inline_level(n) {
            run.push(n.clone());
        } else {
            flush(&mut run, &mut out);
            out.push(n.clone());
        }
    }
    flush(&mut run, &mut out);
    // `text-box-trim` хоста режет его ПЕРВУЮ/ПОСЛЕДНЮЮ отформатированную
    // строку (css-inline-3 §4.2). Строки ушли в анонимные блоки — флаг едет
    // туда, где строка: первому анонимному, если он первый ребёнок, и
    // последнему, если последний (`text-box-trim-multicol-001`).
    if e.style.text_box_trim_start || e.style.text_box_trim_end {
        let flow: Vec<usize> = out
            .iter()
            .enumerate()
            .filter(|(_, n)| !is_blank(n))
            .map(|(i, _)| i)
            .collect();
        let anon = |n: &Node| matches!(n, Node::Element(k) if k.tag == "anon-block");
        if e.style.text_box_trim_start
            && let Some(&i) = flow.first()
            && anon(&out[i])
            && let Node::Element(k) = &mut out[i]
        {
            k.style.text_box_trim_start = true;
        }
        if e.style.text_box_trim_end
            && let Some(&i) = flow.last()
            && anon(&out[i])
            && let Node::Element(k) = &mut out[i]
        {
            k.style.text_box_trim_end = true;
        }
    }
    let mut g = e.clone();
    g.children = out;
    Some(g)
}

/// Клон поддерева, у которого ФИЗИЧЕСКИЕ поля коробки повёрнуты так, что
/// БЛОЧНАЯ ось вертикального письма встаёт на место вертикальной: `width` ↔
/// `height`, стороны — по логическим ролям (css-writing-modes-4 §3.1, §6.4
/// «abstract-to-physical mappings»): новый верх — block-start (левый край у
/// `vertical-lr`, правый у `vertical-rl`), новый низ — block-end, новые лево/право
/// — inline-start/-end (верх/низ при `direction: ltr`).
///
/// Нужен ТОЛЬКО мере стопки колонок: `shape_full` написана в терминах блочного
/// потока (`h` — размер по оси потока, `cuts`/`solid` — смещения от его начала),
/// но читает физические поля. На повёрнутом клоне её `h` — блочный размер, а
/// `flex-direction: row` остаётся строчной осью (в вертикали она вертикальна) —
/// дети ряда стоят рядом, точек разреза между ними нет, как и должно быть.
/// Рисуется по-прежнему ИСХОДНЫЙ элемент: повернуть отрисовку нельзя, вместе с
/// коробкой повернулись бы текст, рамки и фон. Blink делает то же логическими
/// величинами (`BoxStrut`/`LogicalSize` в `block_layout_algorithm.cc`).
///
/// `None` — в поддереве потомок с ДРУГИМ письмом (ортогональный поток,
/// css-writing-modes-4 §7.3, или обратная блочная ось) либо `direction: rtl`:
/// поворотом его мера не выражается, и многоколоночник остаётся на прежнем
/// пути.
pub(crate) fn transpose_tree(c: &Element, rl: bool) -> Option<Element> {
    if c.style.vertical == Some(false)
        || c.style.vertical_rl.is_some_and(|v| v != rl)
        || c.style.rtl == Some(true)
    {
        return None;
    }
    let turn = |s: &crate::style::computed::Sides| crate::style::computed::Sides {
        top: if rl { s.right } else { s.left },
        bottom: if rl { s.left } else { s.right },
        left: s.top,
        right: s.bottom,
    };
    let mut t = c.clone();
    std::mem::swap(&mut t.style.width, &mut t.style.height);
    std::mem::swap(&mut t.style.min_width, &mut t.style.min_height);
    std::mem::swap(&mut t.style.max_width, &mut t.style.max_height);
    t.style.padding = turn(&c.style.padding);
    t.style.margin = turn(&c.style.margin);
    t.style.border_width = turn(&c.style.border_width);
    t.style.inset = turn(&c.style.inset);
    // Видимость рамки — `[верх, право, низ, лево]` (`Computed::borders`).
    let v = c.style.border_visible;
    t.style.border_visible = if rl {
        [v[1], v[2], v[3], v[0]]
    } else {
        [v[3], v[2], v[1], v[0]]
    };
    // Обрезка ПО ОСИ ПОТОКА: в вертикальном письме это `overflow-x`.
    t.style.overflow_y = c.style.overflow_x;
    t.style.overflow_x = c.style.overflow_y;
    // `border-spacing` физическое (`horizontal vertical`), ряды таблицы идут
    // по оси потока: между рядами в вертикали — ГОРИЗОНТАЛЬНАЯ составляющая.
    if let Some((x, y)) = c.style.border_spacing {
        t.style.border_spacing = Some((y, x));
    }
    t.children = c
        .children
        .iter()
        .map(|n| match n {
            Node::Element(k) => transpose_tree(k, rl).map(Node::Element),
            other => Some(other.clone()),
        })
        .collect::<Option<Vec<Node>>>()?;
    Some(t)
}
