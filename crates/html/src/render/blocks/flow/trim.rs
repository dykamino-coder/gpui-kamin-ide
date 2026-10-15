//! `text-box-trim` блочного контейнера (css-inline-3 §text-box-trim): адресация
//! среза абзацу-владельцу строки и отрицательные поля первого/последнего ребёнка.

use crate::dom::Node;
use crate::render::{RenderOpts, out_of_flow};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::line_trim::Trim;
use crate::text::text_box::{blank_text, text_box_line_style, text_box_trim_px};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

/// Узлы-владельцы первой и последней строки, которым контейнер адресовал
/// срез (`line_trim::request`) до раскладки детей.
pub(super) struct Requests {
    start: Option<u64>,
    end: Option<u64>,
}

/// Записать запросы среза для абзацев, держащих первую/последнюю строку.
pub(super) fn register(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    ordered_context: bool,
) -> Requests {
    let mut req = Requests {
        start: None,
        end: None,
    };
    if ordered_context {
        return req;
    }
    for (start, on) in [
        (true, inherited.text_box_trim_start),
        (false, inherited.text_box_trim_end),
    ] {
        if !on {
            continue;
        }
        if let Some((line_style, owner)) =
            text_box_line_style(nodes, inherited, start, inherited.self_node)
        {
            let over = if start {
                root_top(&first_line_root(&line_style), opts) - root_top(&line_style, opts)
            } else {
                0.0
            };
            crate::text::line_trim::request(owner, start, over);
            if start {
                req.start = Some(owner);
            } else {
                req.end = Some(owner);
            }
        }
    }
    req
}

/// Флаги абзаца из запросов блока: начало — у первого абзаца, конец — у
/// абзаца, после которого строк нет.
pub(super) fn para_flags(own: Trim, first: bool, last: bool) -> Trim {
    Trim {
        start: own.start && first,
        end: own.end && last,
        over: own.over,
    }
}

/// Стиль корневой строчной коробки ПЕРВОЙ строки: `::first-line` меняет её
/// шрифт и интерлиньяж (css-pseudo-4 §first-line-styling; Blink `LineStyle()`
/// в `ApplyTextBoxTrim`). Числовой `line-height` хранится множителем и
/// пересчитывается от нового кегля сам.
fn first_line_root(line_style: &Computed) -> Computed {
    let mut c = line_style.clone();
    if let Some(fl) = line_style.first_line.as_deref() {
        let base = match line_style.font_size {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        c.font_size = match (fl.font_size, base) {
            (Some(Len::Em(k)), Some(b)) | (Some(Len::Pct(k)), Some(b)) => Some(Len::Px(k * b)),
            (Some(v), _) => Some(v),
            (None, _) => line_style.font_size,
        };
        if fl.font_family.as_ref().is_some_and(|f| !f.is_empty()) {
            c.font_family = fl.font_family.clone();
        }
        c.line_height = fl.line_height.or(line_style.line_height);
    }
    c
}

/// Расстояние от верха строки струта до базовой линии: полулидинг + подъём.
fn root_top(s: &Computed, opts: &RenderOpts) -> f32 {
    let size = match s.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    };
    let family = s.font_family.clone().unwrap_or_default();
    let (ascent, descent, _) = crate::text::metrics::vmetrics_px(&family, size);
    let line = match s.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        _ => ascent + descent,
    };
    (line - (ascent + descent)) / 2.0 + ascent
}

/// Снять незабранные запросы (строка ушла путём без `blocks_flow`).
pub(super) fn release(req: Requests) {
    if let Some(o) = req.start {
        crate::text::line_trim::cancel(o, true);
    }
    if let Some(o) = req.end {
        crate::text::line_trim::cancel(o, false);
    }
}

/// После `rest` строк больше нет: только пробелы, невидимые и вынесенные из
/// потока узлы — абзац перед ними держит последнюю строку блока.
pub(super) fn rest_is_empty(rest: &[Node]) -> bool {
    rest.iter().all(|n| match n {
        Node::Text(t) => blank_text(t),
        Node::Element(e) => e.style.display == Some(Display::None) || out_of_flow(&e.style),
    })
}

/// Срез отрицательным полем на первом и последнем ребёнке: коробка ужимается
/// ровно на срез от струта до метрики края корневой строчной коробки, а
/// содержимое остаётся на месте. Часть от верха строки до струта снимает
/// сам абзац (`Paragraph::line_padding`).
///
/// Строку ищет `text_box_line_style` по css-pseudo-4: у контейнера с
/// блочным содержимым это первая строка ПЕРВОГО in-flow блочного ребёнка,
/// и если у того строки нет (пустой `<div>`, пустая анонимная коробка) —
/// срезать нечего (`half-leading-block-box-001/003`). «Intervening non-zero
/// padding or borders» — отступы и рамки ПОТОМКОВ между контейнером и
/// строкой (`-004/-005`), а не самого контейнера (`-006`). Край — у корневой
/// строчной коробки найденной строки: `text-box-edge` наследуемое, а явное
/// `auto` на блоке строки перекрывает `ex` контейнера
/// (`not-ignore-nested-text-box-edge`; Blink `AdjustEdges`: kAuto = kText).
pub(super) fn apply(
    out: &mut Vec<AnyElement>,
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
) {
    let vertical = inherited.vertical == Some(true);
    let rl = inherited.vertical_rl == Some(true);
    if inherited.text_box_trim_start
        && let Some((line_style, _)) =
            text_box_line_style(nodes, inherited, true, inherited.self_node)
    {
        let trim = text_box_trim_px(&first_line_root(&line_style), true, opts);
        if trim > 0.0 {
            let first = out.remove(0);
            // Вертикальный блок кладёт детей рядом (`flex_row` у
            // `vertical-lr`, `flex_row_reverse` у `vertical-rl`): блок-старт —
            // левый или правый край (`text-box-trim-half-leading-block-box-002`).
            let holder = match (vertical, rl) {
                (false, _) => div().mt(px(-trim)),
                (true, false) => div().ml(px(-trim)),
                (true, true) => div().mr(px(-trim)),
            };
            out.insert(0, holder.child(first).into_any_element());
        }
    }
    if inherited.text_box_trim_end
        && let Some((line_style, _)) =
            text_box_line_style(nodes, inherited, false, inherited.self_node)
    {
        let trim = text_box_trim_px(&line_style, false, opts);
        if trim > 0.0 {
            let last = out.pop().expect("список не пуст");
            // Блок-конец вертикали: правый край у `vertical-lr`, левый у
            // `vertical-rl`.
            let holder = match (vertical, rl) {
                (false, _) => div().mb(px(-trim)),
                (true, false) => div().mr(px(-trim)),
                (true, true) => div().ml(px(-trim)),
            };
            out.push(holder.child(last).into_any_element());
        }
    }
}
