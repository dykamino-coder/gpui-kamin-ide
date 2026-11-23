//! Вложенный хост полос и его допуски: ruby, формы флоатов.

use super::{has_flow_float, measured_host};
use crate::dom::{Element, Node};
use crate::layout::block::margins::collapse_margins;
use crate::layout::block::struts::zero_len;
use crate::layout::box_kind::is_blank;
use crate::layout::float::band_clearance::supported as band_clear_supported;
use crate::layout::float::band_flow_host::band_flow_rest;
use crate::layout::float::band_host::band_em;
use crate::layout::float::band_nest::{band_nest_block, band_seq};
use crate::style::computed::writing_mode::WritingMode;

/// Измеряемый хост, начатый блоком потока с флоатами внутри (шаг F7): сам
/// блок — `Kind::Nest`, за ним — хвост как у `band_host_m` до флоата или
/// `clear`. Без братьев за блоком хост не нужен: флоаты внутри влияют
/// только на его собственное содержимое, и его раскладывает он сам.
pub(crate) fn band_host_nested(
    nodes: &[Node],
    i: usize,
    em: f32,
    parent_bfc: bool,
    cb: crate::layout::float::FloatCb,
    vcx: crate::values::ValueCx<'_>,
) -> Option<(Element, usize)> {
    // Без следующей коробки обычному блоку не нужен общий хост. Проверяем
    // это до поиска флоатов: иначе цепочка одиночных блоков сканируется N² раз.
    if !parent_bfc && nodes[i + 1..].iter().all(is_blank) {
        return None;
    }
    let wm = cb.wm;
    let Node::Element(c) = &nodes[i] else {
        return None;
    };
    if !has_flow_float(c) || !band_nest_ok(c, em, wm, cb.margin_basis, vcx) {
        return None;
    }
    let mut j = i + 1;
    let mut rest: Vec<Node> = vec![];
    while j < nodes.len() {
        if let Node::Element(next) = &nodes[j]
            && (next
                .style
                .used()
                .reset_box_layout
                .float
                .is_some_and(|f| f != 0)
                || (next.style.used().reset_box_layout.clear.is_some()
                    && !band_clear_supported(next)))
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
    let rest = band_flow_rest(rest, em, wm, cb.margin_basis, vcx)?;
    if let Some(Node::Element(next)) = nodes[j..].iter().find(|n| !is_blank(n))
        && next.style.used().reset_box_layout.clear.is_some()
        && !band_clear_supported(next)
        && !zero_len(next.style.used().reset_box_layout.margin.top)
    {
        return None;
    }
    let mut children = vec![Node::Element(c.clone())];
    children.extend(rest);
    let host = measured_host(
        crate::dom::ShapeFlow {
            em: Some(em),
            cbh: cb.height,
            cbw: cb.width,
            wm,
            icb: cb.icb,
            ..Default::default()
        },
        children,
    );
    Some((host, j))
}

/// Блок с флоатами или коробками своего контекста внутри годится в
/// `Kind::Nest` целиком — со всеми потомками (шаг F7).
pub(crate) fn band_nest_ok(
    c: &Element,
    em: f32,
    wm: WritingMode,
    cb: Option<f32>,
    vcx: crate::values::ValueCx<'_>,
) -> bool {
    if !band_nest_block(c, em, wm) {
        return false;
    }
    let Some(inner_em) = band_em(&c.style.used(), em) else {
        return false;
    };
    // `band_seq` validates each float and every non-float chunk through
    // `band_flow_rest`, including nested blocks. Walking the returned sequence
    // again would recursively validate each descendant twice at every depth.
    band_seq(
        collapse_margins(
            &c.children,
            false,
            cb,
            crate::layout::block::margins::MarginCx::new(vcx),
        ),
        inner_em,
        wm,
        cb,
        c.style.cascaded().inh.text_flow.preserve_newlines == Some(true),
        vcx,
    )
    .is_some()
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
