//! Подготовка обтекания: узлы с поднятыми флоатами, ранний выход без флоатов,
//! бандовый хост пробега.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::block::struts::float_only_wrapper;
use crate::layout::float::band_measured::{band_host_m, band_host_nested};
use crate::layout::float::clear::bfc_no_fit;
use crate::layout::float::initial_letter::{inline_float_host, split_leading_float};
use crate::layout::float::{FloatCb, band_clearance, float_clear_scope, inline_floats};
use crate::render::{inline_level, is_blank, out_of_flow, phantom_inline};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

pub(super) fn unfloated_flow(
    cb_top_open: bool,
    em: f32,
    measured_ok: bool,
    parent_bfc: bool,
    cb: FloatCb,
    nodes: &mut Vec<Node>,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<Vec<Node>> {
    let floated = nodes.iter().any(|n| match n {
        Node::Element(e) => e.style.float.is_some_and(|f| f != 0),
        Node::Text(_) => false,
    });
    if !floated {
        // Флоатов среди прямых детей нет, но они есть ВНУТРИ блока потока, и
        // за ним идут братья — им эти флоаты видны (одно пространство
        // исключений на БФК, шаг F7: `new-fc-separates-from-float`,
        // `floats-bfc-003`). Хост начинается с такого блока.
        if measured_ok {
            for i in 0..nodes.len() {
                if let Some((mut host, j)) = band_host_nested(nodes, i, em, parent_bfc, cb, vcx) {
                    let mut out: Vec<Node> = nodes[..i].to_vec();
                    band_clearance::mark_start(&mut host, cb_top_open, &out);
                    out.push(Node::Element(host));
                    out.extend(nodes[j..].iter().cloned());
                    return Some(out);
                }
            }
        }
        return Some(std::mem::take(nodes));
    }
    None
}

pub(super) fn prepare_float_nodes(
    nodes: Vec<Node>,
    parent: &Computed,
    cb_top_open: bool,
    cb_px: Option<f32>,
    vcx: ValueCx<'_>,
) -> Vec<Node> {
    // CSS Display 3 §2.5: an element with `display: contents` generates no
    // box, so `float` does not apply to it (`display-contents-float-001`).
    let nodes: Vec<Node> = nodes
        .into_iter()
        .map(|n| match n {
            Node::Element(mut e) if e.style.display == Some(Display::Contents) => {
                e.style.float = None;
                Node::Element(e)
            }
            Node::Element(mut e) if e.style.float.is_some_and(|f| f != 0) => {
                // Font-relative margins use the float's computed font, before
                // the band planner takes ownership of its margin box.
                e.style.margin =
                    crate::style::cascade::inherit::inherit_unpainted(parent, &e.style, vcx).margin;
                Node::Element(e)
            }
            n => n,
        })
        .collect();
    let nodes = inline_floats::lift(float_clear_scope::used(nodes, parent), parent, vcx);
    // Флоат, записанный ВНУТРИ строчной коробки, принадлежит не ей, а
    // ближайшему блочному предку (§10.1, §9.5.1 п.1). Строчная обёртка, в
    // которой кроме флоата ничего нет, снимается ЗДЕСЬ — ДО проверки
    // `floated`: иначе `wrap_floats` про такой флоат не узнает вовсе и выйдет
    // первой же строкой, а флоат уедет в абзац вместе с обвязкой `<span>`.
    let nodes: Vec<Node> = nodes
        .into_iter()
        .map(|n| {
            let hoisted = match &n {
                Node::Element(e) => inline_float_host(e),
                Node::Text(_) => None,
            };
            hoisted.map_or(n, Node::Element)
        })
        .collect();
    // A float at the very START of an inline wrapper that also holds other
    // content (only collapsible white space before it): it is placed before
    // anything of the first line (CSS 2.1 §9.5.1 rule 1 — its top is the
    // top of the line it occurs on, and nothing of that line precedes it),
    // so it is laid out exactly like a float written just before the
    // wrapper. `below-float`: `<span> <div float 100%> x</span>` must push
    // `x` (and its text-indent) below the float; the float was lost.
    let nodes: Vec<Node> = nodes
        .into_iter()
        .flat_map(|n| match &n {
            Node::Element(e) => match split_leading_float(e) {
                Some((float, rest)) => vec![Node::Element(float), Node::Element(rest)],
                None => vec![n],
            },
            Node::Text(_) => vec![n],
        })
        .collect();
    // Примыкающие флоаты во ВЛОЖЕННОЙ обёртке (Blink
    // `HasClearancePastAdjoiningFloats`; §9.5 для нового контекста): первая в
    // потоке — обёртка из одних флоатов (`float_only_wrapper`, §10.6.3 её уже
    // обнулил), следом разделитель — `clear` в сторону её флоатов или коробка
    // своего контекста без места рядом. Верх блока открыт, значит флоаты
    // ПРИМЫКАЮТ: разделитель встаёт ровно под их низом, поле не участвует
    // («No matter how large the margin is, it should still be just below the
    // float» — `adjoining-float-before-clearance`). Обёртке возвращается
    // высота `auto`: наша раскладка держит в ней флоат лоном, то есть его
    // высотой; полю разделителя — ноль. Цепь `leading_chain` поле такого
    // разделителя наверх уже не поднимала.
    let mut nodes = nodes;
    if cb_top_open
        && let Some(a) = nodes.iter().position(|n| !is_blank(n))
        && let Node::Element(w) = &nodes[a]
        && let Some((left, right, min_w)) = float_only_wrapper(w)
        && let Some(b) =
            (a + 1..nodes.len()).find(|&k| !is_blank(&nodes[k]) && !phantom_inline(&nodes[k]))
        && let Node::Element(n) = &nodes[b]
        && (match n.style.clear {
            Some(0) => true,
            Some(c) if c < 0 => left,
            Some(_) => right,
            None => false,
        } || bfc_no_fit(n, min_w, cb_px))
    {
        if let Node::Element(w) = &mut nodes[a] {
            w.style.height = None;
        }
        if let Node::Element(n) = &mut nodes[b] {
            n.style.margin.top = Some(Len::Px(0.0));
        }
    }
    nodes
}

/// The measured float host for the run starting at node `i`, if the run
/// and its tail are expressible in it; `None` keeps the run on the row path.
pub(super) fn host_for_run(
    em: f32,
    measured_ok: bool,
    cb: FloatCb,
    nodes: &[Node],
    out: &[Node],
    i: usize,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<(Element, usize, Option<usize>, Vec<Node>)> {
    if !measured_ok {
        return None;
    }
    // CSS 2.1 §9.5.1 rule 6: a float in the middle of a line («Hello<float>
    // Kitty») sits on that line. The inline content collected since the last
    // block or `<br>` goes into the host as the start of its first run, and
    // the float onto its line (`floats-placement-vertical-001a`); preceding
    // atoms included (`floats-030`: text, an image, then the float).
    let text_at = out
        .iter()
        .rposition(|n| match n {
            Node::Text(_) => false,
            Node::Element(c) => {
                !inline_level(c)
                    || c.tag == "br"
                    || out_of_flow(&c.style)
                    || c.marks.shape_flow.is_some()
            }
        })
        .map_or(0, |p| p + 1);
    let text_lead = out[text_at..].iter().any(|n| !is_blank(n));
    text_lead
        .then(|| band_host_m(nodes, i, em, &out[text_at..], cb, vcx))
        .flatten()
        .map(|(h, n, l)| (h, n, Some(text_at), l))
        .or_else(|| band_host_m(nodes, i, em, &[], cb, vcx).map(|(h, n, l)| (h, n, None, l)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn wrap_into_host(
    parent: &Computed,
    cb_top_open: bool,
    parent_bfc: bool,
    cell_bfc: bool,
    nodes: &[Node],
    out: &mut Vec<Node>,
    i: &mut usize,
    (mut host, next, took_lead, lifted): (Element, usize, Option<usize>, Vec<Node>),
) {
    {
        // CSS 2.1 §10.6.3: ordinary blocks count in-flow boxes, not floats.
        // A complete unfragmented suffix needs no later float bands, and
        // no later box of the enclosing context may see them (`float_tail`;
        // also excludes float boxes whose own style lost `float` here).
        if !parent_bfc
            && !cell_bfc
            && next == nodes.len()
            && parent.float_tail
            && !parent.in_multicol
        {
            host.marks.inflow_height = true;
        }
        if let Some(at) = took_lead {
            out.truncate(at);
        }
        // Хост измеряемый и до него в блоке ничего нет — первая строка
        // блока внутри хоста: слой `::first-line` едет с ним
        // (`band_kids` отдаёт его первому строчному прогону).
        // Внепоточные соседи (абсолюты, флоаты) строк не образуют
        // (`below-float3`: абсолют перед флоатом).
        if host.marks.shape_flow.is_some()
            && out
                .iter()
                .all(|n| is_blank(n) || matches!(n, Node::Element(c) if out_of_flow(&c.style)))
        {
            host.first_line = parent.first_line.as_deref().cloned().map(Into::into);
        }
        band_clearance::mark_start(&mut host, cb_top_open, &*out);
        out.push(Node::Element(host));
        out.extend(lifted);
        *i = next;
    }
}

#[cfg(test)]
mod tests;
