//! Хвост измеренного хоста полос (band_host_m_tail).

use super::{band_piece_m, measured_host};
use crate::dom::{Element, Node};
use crate::layout::block::struts::zero_len;
use crate::layout::float::FloatCb;
use crate::layout::float::band_clearance::supported as band_clear_supported;
use crate::layout::float::band_flow_host::band_flow_rest_lift;
use crate::render::is_blank;
use crate::style::computed::Computed;

/// Хвост `band_host_m`: хост из собранных флоатов, хвоста и щупов.
#[allow(clippy::too_many_arguments)]
pub(super) fn band_host_m_tail(
    nodes: &[Node],
    j: usize,
    em: f32,
    lead: &[Node],
    floaters: Vec<Element>,
    rest: Vec<Node>,
    probes: Vec<(usize, usize, Vec<Node>)>,
    cb: FloatCb,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<(Element, usize, Vec<Node>)> {
    // Хвост: куски своего контекста, блоки потока и строчные прогоны
    // (шаг F4: строки блоков потока режутся полосами, `band_flow::Kind::Flow`).
    let mut lifted: Vec<Node> = vec![];
    // Строчное содержимое перед флоатом в той же строке (`wrap_floats`):
    // оно — начало первого прогона хоста, флоаты встают на его строку.
    let lead_probe = (!lead.is_empty()).then_some(());
    let probe_nodes: Vec<Node> = probes
        .into_iter()
        .flat_map(|(a, b, pre)| {
            let (base, line) = super::lead::split(pre, cb.preserve_newlines);
            let probe = |children: Vec<Node>, base: bool| {
                Node::Element(Element::synthetic(
                    "div",
                    Computed::default(),
                    crate::dom::Marks {
                        anon: true,
                        lead: Some(crate::dom::LeadProbe {
                            base,
                            from: a,
                            to: b,
                        }),
                        ..Default::default()
                    },
                    children,
                ))
            };
            let mut v = vec![probe(line, false)];
            if let Some(base) = base {
                v.push(probe(base, true));
            }
            v
        })
        .collect();
    let rest = if lead.is_empty() {
        rest
    } else {
        let mut r = lead.to_vec();
        r.extend(rest);
        r
    };
    let rest = band_flow_rest_lift(rest, em, Some(&mut lifted), cb.wm, cb.margin_basis, vcx)?;
    // С содержимым перед флоатом первый кусок хвоста — его прогон: иначе
    // флоат не на той строке.
    if lead_probe.is_some() && !matches!(rest.first(), Some(Node::Element(c)) if c.marks.anon) {
        return None;
    }
    // Блок потока или строчный прогон в хвосте (не кусок своего контекста).
    let flows = rest
        .iter()
        .any(|n| matches!(n, Node::Element(_)) && band_piece_m(n, em, cb.wm).is_none());
    // За хвостом — очищающая коробка с верхним полем: её поле и clearance
    // (§9.5.2) решаются в паре с высотой хвоста, а хост их не видит —
    // остаётся распорке флекс-ряда (`adjoining-float-nested-forced-clearance-003`).
    if flows
        && let Some(Node::Element(next)) = nodes[j..].iter().find(|n| !is_blank(n))
        && next.style.clear.is_some()
        && !band_clear_supported(next)
        && !zero_len(next.style.margin.top)
    {
        return None;
    }
    // Высота, ширина и письмо содержащего блока: у хоста своих нет.
    let count = floaters.len();
    let mut children: Vec<Node> = floaters.into_iter().map(Node::Element).collect();
    children.extend(rest);
    children.extend(probe_nodes);
    let host = measured_host(
        crate::dom::ShapeFlow {
            count,
            em: Some(em),
            cbh: cb.height,
            cbw: cb.width,
            wm: cb.wm,
            icb: cb.icb,
            ..Default::default()
        },
        children,
    );
    Some((host, j, lifted))
}
