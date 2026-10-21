//! Гейт измеряемого хоста полос: годится ли пробег флоатов с хвостом из кусков для
//! общего плана `band_flow::BandFlow`, где размеры, которых нет в стиле, меряет раскладка.
//! `band_host_m` собирает пробег флоатов, щупы строчного содержимого перед ними и хвост;
//! `band_piece_m` и `band_float_m` решают, годится ли кусок или флоат (литералы стиля —
//! `band_host`), иначе хост отменяется. Подмодули: строки перед флоатом (`lead`), хвост
//! (`tail`), вложенные блоки с флоатами (`nested`).

use crate::dom::{Element, Node};
use crate::layout::float::FloatCb;
use crate::layout::float::band_clearance::supported as band_clear_supported;
use crate::layout::float::band_flow_host::band_orthogonal;
use crate::layout::float::band_host::{BandPiece, band_margins, band_piece};
use crate::render::{block_level_in_flow, is_blank, out_of_flow, own_context, replaced_tag};
use crate::style::computed::Display;
use crate::style::computed::writing_mode::WritingMode;
mod lead;
pub(super) use lead::probes as lead_probes;
mod tail;
use tail::band_host_m_tail;
mod nested;
pub(super) use nested::band_host_nested;
pub(super) use nested::band_nest_ok;
pub(super) use nested::has_ruby;

/// Кусок хвоста измеряемого хоста: `Some(true)` — коробка, флоаты не
/// перекрывающая (§9.5, последний абзац), `Some(false)` — распорка, `None` —
/// не годится (хост отменяется).
///
/// В отличие от `band_piece` размеры коробки своего контекста в стиле не
/// нужны: ширину окна и высоту из содержимого даёт пробная раскладка
/// (`band_flow::plan`). Атомы строки сюда НЕ пускаются: они делят строку, а
/// здесь каждый кусок берёт своё окно.
pub(super) fn band_piece_m(n: &Node, em: f32, wm: WritingMode) -> Option<bool> {
    match band_piece(n) {
        Some(BandPiece::Strut) => return Some(false),
        Some(BandPiece::Atom) => return None,
        _ => {}
    }
    let Node::Element(c) = n else {
        return None;
    };
    if matches!(
        c.style.position,
        Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
    ) || c.style.float.is_some_and(|f| f != 0)
        || (c.style.clear.is_some() && !band_clear_supported(c))
        || matches!(
            c.style.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
                | Some(Display::TableCell)
                | Some(Display::None)
        )
        // Строчный по природе тег строчен, только пока `display` не задан:
        // `<img style="display:block">` — блочного уровня.
        || (c.inline && c.style.display.is_none())
        // Таблица и замещаемый блочного уровня §9.5 названы прямо; тег
        // `<table>` вида в стиле не несёт — его строит сборщик таблиц по тегу
        // (`"table" =>` в `element`); `<img style="display:block">` своего
        // контекста не заводит, но флоаты не перекрывает (`floats-039`).
        // Writing Modes 4 section 3.2: an orthogonal block establishes an
        // independent formatting context. Its measured border box uses the
        // same opportunity search as other non-overlapping pieces.
        || !(own_context(c)
            || band_orthogonal(&c.style, wm)
            || c.tag == "table"
            || (block_level_in_flow(c) && replaced_tag(c)))
    {
        return None;
    }
    band_margins(&c.style, em)?;
    Some(true)
}

/// Измеряемый бандовый хост:
/// тот же пробег флоатов и хвост из кусков, что у `band_host`, но размеры,
/// которых нет в стиле, меряет раскладка (`band_flow::BandFlow`).
///
/// Гейт (все условия разом):
///
/// * у каждого флоата пробега поля разрешимы (`band_margins`); размеры любые
///   — shrink-to-fit §10.3.5 считает проба;
/// * хвост — только пустой текст и куски `band_piece_m`;
/// * одинокий флоат с пустым хвостом полосам не нужен (как у `band_host`).
///
/// Ширина содержащего блока не требуется вовсе: её отдаёт замер.
pub(super) fn band_host_m(
    nodes: &[Node],
    i: usize,
    em: f32,
    lead: &[Node],
    cb: FloatCb,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<(Element, usize, Vec<Node>)> {
    let mut floaters: Vec<Element> = vec![];
    let mut j = i;
    while j < nodes.len() {
        if is_blank(&nodes[j]) {
            j += 1;
            continue;
        }
        let Node::Element(next) = &nodes[j] else {
            break;
        };
        if next.style.float.is_none_or(|f| f == 0) {
            break;
        }
        band_float_m(next, em)?;
        floaters.push(next.clone());
        j += 1;
    }
    if floaters.is_empty() {
        return None;
    }
    // Щупы ширины строчного содержимого перед флоатами (`Kid::lead`):
    // (номера флоатов, узлы перед ними в той же строке). Флоаты пробега
    // стоят после `lead` (`wrap_floats`).
    let mut probes: Vec<(usize, usize, Vec<Node>)> = vec![];
    if !lead.is_empty() {
        probes.push((0, floaters.len(), lead.to_vec()));
    }
    let mut rest: Vec<Node> = vec![];
    while j < nodes.len() {
        // С шагом F6 очищающая коробка остаётся в хосте: clearance считают
        // полосы (`band_flow::plan`), а не распорка флекс-ряда.
        if let Node::Element(next) = &nodes[j]
            && (next.style.float.is_some_and(|f| f != 0)
                || (next.style.clear.is_some() && !band_clear_supported(next)))
        {
            // Флоат дальше по той же строке прогона («BEF<float>Inner
            // <float>AFTER», `::after { float: right }` за текстом): пока
            // перед ним только строчное содержимое без разрывов, он встаёт
            // на ту же строку (правило 6 §9.5.1) — в этот же хост, со своим
            // щупом ширины набранного до него. Отдельный хост начинал бы
            // его с новой строки (`before-after-floated-001`).
            // Атомы в набранном — нет: щуп высоты их ряда (`FlowRow`) не
            // спускает строки под вырезы ранних флоатов, и второй флоат
            // вставал прямо под первым (`shape-outside-border-box-001-ref`:
            // 7.02 с атомами в щупе).
            let inline_only = rest.iter().all(|n| match n {
                Node::Text(_) => true,
                // A `<br clear>` moves what follows below the floats: a
                // float after it starts a run of its own.
                Node::Element(c) if c.tag == "br" => c.style.clear.is_none(),
                Node::Element(c) => {
                    !block_level_in_flow(c)
                        && !out_of_flow(&c.style)
                        && band_piece(n) != Some(BandPiece::Atom)
                }
            });
            if next.style.float.is_some_and(|f| f != 0)
                && inline_only
                && rest.iter().any(|n| !is_blank(n))
                && band_float_m(next, em).is_some()
            {
                let mut pre = lead.to_vec();
                pre.extend(rest.iter().cloned());
                probes.push((floaters.len(), floaters.len() + 1, pre));
                floaters.push(next.clone());
                j += 1;
                continue;
            }
            break;
        }
        rest.push(nodes[j].clone());
        j += 1;
    }
    band_host_m_tail(nodes, j, em, lead, floaters, rest, probes, cb, vcx)
}

/// The synthetic node of a measured float host: the element dispatcher hands it to
/// `band_flow_host`, which lays its children out with `band_flow::BandFlow`.
fn measured_host(flow: crate::dom::ShapeFlow, children: Vec<Node>) -> Element {
    let marks = crate::dom::Marks {
        shape_flow: Some(flow),
        ..Default::default()
    };
    Element::synthetic("shape-flow", Default::default(), marks, children)
}

/// Годится ли флоат в измеряемый хост: `None` — хост отменяется.
pub(super) fn band_float_m(next: &Element, em: f32) -> Option<()> {
    band_margins(&next.style, em)?;
    Some(())
}

/// Есть ли у блока флоат среди потомков обычного потока (сквозь блоки, не
/// заводящие своего контекста).
fn has_flow_float(c: &Element) -> bool {
    c.children.iter().any(|n| match n {
        Node::Text(_) => false,
        Node::Element(k) => {
            k.style.float.is_some_and(|f| f != 0)
                || (block_level_in_flow(k) && !own_context(k) && has_flow_float(k))
        }
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod construction_tests;

#[cfg(test)]
mod orthogonal_tests;
