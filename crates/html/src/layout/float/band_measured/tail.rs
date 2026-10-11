//! Хвост измеренного хоста полос (band_host_m_tail).

use super::{band_piece_m, host_floats_shaped};
use crate::dom::{Element, Node};
use crate::layout::block::struts::zero_len;
use crate::layout::float::band_clearance::supported as band_clear_supported;
use crate::layout::float::band_flow_host::{BAND_CBH, BAND_CBW, band_flow_rest_lift};
use crate::render::is_blank;
use crate::style::computed::Computed;

/// Хвост `band_host_m`: хост из собранных флоатов, хвоста и щупов.
#[allow(clippy::too_many_arguments)]
pub(super) fn band_host_m_tail(
    nodes: &[Node],
    i: usize,
    j: usize,
    em: f32,
    lead: &[Node],
    floaters: Vec<Element>,
    rest: Vec<Node>,
    probes: Vec<(usize, usize, Vec<Node>)>,
) -> Option<(Element, usize, Vec<Node>)> {
    // Одинокий флоат без хвоста полосам не нужен — если перед ним в строке
    // ничего нет: флоат ПОСЛЕ текста («Inner<float>») встаёт на его строку
    // только в хосте.
    if floaters.len() < 2 && !rest.iter().any(|n| !is_blank(n)) && lead.is_empty() {
        return None;
    }
    // Хвост: куски своего контекста, блоки потока и строчные прогоны
    // (шаг F4: строки блоков потока режутся полосами, `band_flow::Kind::Flow`).
    let mut lifted: Vec<Node> = vec![];
    // Строчное содержимое перед флоатом в той же строке (`wrap_floats`):
    // оно — начало первого прогона хоста, флоаты встают на его строку.
    let lead_probe = (!lead.is_empty()).then_some(());
    let probe_nodes: Vec<Node> = probes
        .into_iter()
        .flat_map(|(a, b, pre)| {
            // Последний `<br>` верхнего уровня делит набранное: до него —
            // основание (`lead-base`), после — строка флоата (`lead-for`).
            let cut = pre
                .iter()
                .rposition(|n| matches!(n, Node::Element(c) if c.tag == "br"))
                .map(|p| p + 1);
            let (base, line) = match cut {
                Some(p) => (Some(pre[..p].to_vec()), pre[p..].to_vec()),
                None => (None, pre),
            };
            let probe = |children: Vec<Node>, key: &str| {
                Node::Element(Element {
                    list_item: None,
                    node_id: 0,
                    anim: None,
                    tag: "div".into(),
                    style: Computed::default(),
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children,
                    attrs: vec![
                        ("anon".into(), "1".into()),
                        ("lead-probe".into(), "1".into()),
                        (key.into(), format!("{a}-{b}")),
                    ],
                    inline: false,
                })
            };
            let mut v = vec![probe(line, "lead-for")];
            if let Some(base) = base {
                v.push(probe(base, "lead-base"));
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
    let rest = band_flow_rest_lift(rest, em, Some(&mut lifted))?;
    // С содержимым перед флоатом первый кусок хвоста — его прогон: иначе
    // флоат не на той строке.
    if lead_probe.is_some()
        && !matches!(rest.first(), Some(Node::Element(c)) if c.attr("anon") == Some("1"))
    {
        return None;
    }
    // Блок потока или строчный прогон в хвосте (не кусок своего контекста).
    let flows = rest
        .iter()
        .any(|n| matches!(n, Node::Element(_)) && band_piece_m(n, em).is_none());
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
    // Флоат с `shape-outside` рядом со СТРОКАМИ: строки обтекают ФОРМУ, а
    // полосы держат только прямоугольник margin-box (css-shapes-1 §1) —
    // такому пробегу место на живом пути форм `shape_flow`. Коробкам своего
    // контекста форма не важна: они обходят margin-box.
    if flows && host_floats_shaped(&nodes[i..j]) {
        return None;
    }
    let mut host = Element {
        list_item: None,
        node_id: 0,
        anim: None,
        tag: "shape-flow".into(),
        style: Computed::default(),
        hover: None,
        first_letter: None,
        first_line: None,
        children: Vec::new(),
        attrs: vec![
            ("count".into(), floaters.len().to_string()),
            ("em".into(), em.to_string()),
            // Высота содержащего блока (`BAND_CBH`): `shape-flow` своей нет.
            (
                "cbh".into(),
                BAND_CBH
                    .with(std::cell::Cell::get)
                    .map_or(String::new(), |v| v.to_string()),
            ),
            // И ширина (`BAND_CBW`) — блочный размер в вертикальном письме.
            (
                "cbw".into(),
                BAND_CBW
                    .with(std::cell::Cell::get)
                    .map_or(String::new(), |v| v.to_string()),
            ),
            // Метка измеряемого хоста: `shape_flow` отдаёт его `band_flow`.
            ("bands".into(), "m".into()),
        ],
        inline: false,
    };
    host.children = floaters.into_iter().map(Node::Element).collect();
    host.children.extend(rest);
    host.children.extend(probe_nodes);
    Some((host, j, lifted))
}
