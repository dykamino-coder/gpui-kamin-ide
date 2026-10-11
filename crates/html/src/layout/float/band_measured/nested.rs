//! Вложенный хост полос и его допуски: ruby, формы флоатов.

use super::{band_piece_m, has_flow_float};
use crate::dom::{Element, Node};
use crate::layout::block::margins::collapse_margins;
use crate::layout::block::struts::zero_len;
use crate::layout::float::band_clearance::supported as band_clear_supported;
use crate::layout::float::band_flow_host::{band_flow_block, band_flow_rest};
use crate::layout::float::band_host::band_em;
use crate::layout::float::band_nest::{band_nest_block, band_seq};
use crate::render::is_blank;
use crate::style::computed::Computed;

/// Измеряемый хост, начатый блоком потока с флоатами внутри (шаг F7): сам
/// блок — `Kind::Nest`, за ним — хвост как у `band_host_m` до флоата или
/// `clear`. Без братьев за блоком хост не нужен: флоаты внутри влияют
/// только на его собственное содержимое, и его раскладывает он сам.
pub(crate) fn band_host_nested(
    nodes: &[Node],
    i: usize,
    em: f32,
    parent_bfc: bool,
) -> Option<(Element, usize)> {
    let Node::Element(c) = &nodes[i] else {
        return None;
    };
    if !has_flow_float(c) || !band_nest_ok(c, em) {
        return None;
    }
    let mut j = i + 1;
    let mut rest: Vec<Node> = vec![];
    while j < nodes.len() {
        if let Node::Element(next) = &nodes[j]
            && (next.style.float.is_some_and(|f| f != 0)
                || (next.style.clear.is_some() && !band_clear_supported(next)))
        {
            break;
        }
        rest.push(nodes[j].clone());
        j += 1;
    }
    // Без братьев за блоком хост нужен только корню БФК: §10.6.7 требует
    // охватить флоаты высотой (Blink `block_layout_algorithm.cc:1309-1315`,
    // гейт `IsNewFormattingContext`; Servo `BlockFormattingContext::layout`,
    // `flow/mod.rs:460-465`), а блок, обнулённый §10.6.3, их не держит
    // (`letter-spacing-206`). Обычному блоку флоаты внутри влияют только на
    // его собственное содержимое — его раскладывает он сам.
    if !rest.iter().any(|n| !is_blank(n)) && !parent_bfc {
        return None;
    }
    let rest = band_flow_rest(rest, em)?;
    if let Some(Node::Element(next)) = nodes[j..].iter().find(|n| !is_blank(n))
        && next.style.clear.is_some()
        && !band_clear_supported(next)
        && !zero_len(next.style.margin.top)
    {
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
            ("count".into(), "0".into()),
            ("em".into(), em.to_string()),
            ("bands".into(), "m".into()),
        ],
        inline: false,
    };
    host.children = vec![Node::Element(c.clone())];
    host.children.extend(rest);
    Some((host, j))
}

/// Блок с флоатами или коробками своего контекста внутри годится в
/// `Kind::Nest` целиком — со всеми потомками (шаг F7).
pub(crate) fn band_nest_ok(c: &Element, em: f32) -> bool {
    if !band_nest_block(c, em) {
        return false;
    }
    let Some(inner_em) = band_em(&c.style, em) else {
        return false;
    };
    let Some(seq) = band_seq(collapse_margins(&c.children, false), inner_em) else {
        return false;
    };
    seq.iter().all(|n| match n {
        Node::Text(_) => true,
        Node::Element(k) => {
            k.style.float.is_some_and(|f| f != 0)
                || band_piece_m(n, inner_em).is_some()
                || k.attr("anon") == Some("1")
                || band_flow_block(k, inner_em)
                || band_nest_ok(k, inner_em)
        }
    })
}

/// Есть ли в поддереве руби (`<ruby>`, `<rt>`).
pub(crate) fn has_ruby(n: &Node) -> bool {
    match n {
        Node::Text(_) => false,
        Node::Element(e) => {
            matches!(e.tag.as_str(), "ruby" | "rt" | "rtc" | "rb")
                || e.children.iter().any(has_ruby)
        }
    }
}

/// Есть ли среди флоатов пробега (`run` — узлы от первого флоата до конца
/// хвоста) флоат с `shape-outside`.
pub(super) fn host_floats_shaped(run: &[Node]) -> bool {
    run.iter().any(|n| {
        matches!(n, Node::Element(c)
            if c.style.float.is_some_and(|f| f != 0) && c.style.shape_outside.is_some())
    })
}
