//! Текст строки и обрезка пробелов на разрывах для формы прогона.

use crate::dom::Node;
use crate::layout::fragment::LineFrame;
use crate::render::out_of_flow;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Текст строчного содержимого для меры строк: `<br>` — `\n`, пробелы
/// схлопнуты (css-text-3 §4.1.1). `None` — среди детей есть то, что строку
/// меняет сверх голого текста (атом, свой шрифт, отбивка, внепоточный).
pub(crate) fn line_text(nodes: &[Node]) -> Option<String> {
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

/// Срез строк у разрыва колонки с одной стороны: ближайшая к строке коробка
/// со срезом этой стороны решает — многоколоночник (первый кадр: каждая
/// колонка — его фрагментаинер) или коробка с `box-decoration-break: clone`
/// режут у каждого разрыва, коробка `slice` — только у своих первой/последней
/// строки, и срез многоколоночника под ней не действует
/// (`text-box-trim-multicol-004`: блок `trim-start` под `trim-both` — низ
/// первой колонки срезан, верх второй нет; `-005` — наоборот).
pub(crate) fn brk_trim(frames: &[LineFrame], side: impl Fn(&Computed) -> bool) -> bool {
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
pub(crate) fn trim_amount(s: &Computed, size: f32, lh: f32, start: bool) -> f32 {
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
