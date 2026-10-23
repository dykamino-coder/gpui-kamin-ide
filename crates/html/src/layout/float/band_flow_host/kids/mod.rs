//! Дети полос из узлов потока (band_kids): вид, поля, заголовок первой строки.

use super::build_band_kid;
use crate::dom::{Element, Node};
use crate::layout::float::band_host::{band_em, band_margins};
use crate::layout::float::band_measured::band_piece_m;
use crate::layout::float::shape_exclusion::measured_shape;
use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use crate::style::computed::writing_mode::WritingMode;
mod deferred;
mod parts;
use deferred::deferred;
pub(super) use parts::band_kid_kind;

/// Дети одного содержащего блока измеряемого хоста. `count` первых
/// элементов — флоаты пробега; дальше флоатом считается всякий элемент с
/// `float` (дети `Kind::Nest`, шаг F7).
pub(crate) fn band_kids(
    nodes: &[Node],
    count: usize,
    inherited: &Computed,
    opts: &RenderOpts,
    em: f32,
    start_open: bool,
    wm: WritingMode,
) -> Vec<crate::layout::float::band_flow::Kid> {
    use crate::layout::float::band_flow::{Kid, Nest};
    let mut kids: Vec<Kid> = vec![];
    // Письмо содержащего блока: план хоста — в логических осях, поля
    // переводятся в (block-start, inline-end, block-end, inline-start)
    // по таблице css-writing-modes-4 :1877-1888 (`vertical-rl`: block-start
    // — право, inline-start — верх; `vertical-lr`: block-start — лево).
    let vert = (inherited.wm().vertical()).then_some(inherited.wm().block_rl());
    // Был ли уже ребёнок потока со строками (не флоат и не распорка).
    let mut seen_inflow = false;
    // Строчное содержимое перед флоатами пробега (`lead-probe`,
    // `band_host_m`): щуп ширины, сам он не рисуется — его узлы идут в
    // начале первого прогона.
    // При `white-space: nowrap` мягких разрывов нет, и строка флоата — весь
    // первый прогон: щуп — он целиком, флоат влезает рядом, только если
    // влезает вся строка, иначе уходит под неё (Blink откладывает флоат до
    // возможности разрыва; `float-nowrap-8` против эталона
    // `float-nowrap-1`: флоат после всей строки).
    let has_lead = nodes
        .iter()
        .any(|n| matches!(n, Node::Element(p) if p.marks.lead.is_some()));
    let nowrap = inherited.nowrap == Some(true);
    let initial_letter = nodes
        .iter()
        .any(|n| matches!(n, Node::Element(e) if e.marks.initial_letter));
    let mk_build = |p: &Element| -> crate::layout::float::band_flow::Build {
        let p = p.clone();
        let inherited = inherited.clone();
        deferred(opts, false, move |opts, _cb, _avail, _shapes, _h| {
            opts.cx.build.element(&p, &inherited, opts)
        })
    };
    // Первый прогон — щуп для всех флоатов при `nowrap`.
    let nowrap_probe: Option<crate::layout::float::band_flow::Build> = (nowrap && has_lead)
        .then(|| {
            nodes.iter().find_map(|n| match n {
                Node::Element(p) if p.marks.anon && p.marks.lead.is_none() => Some(mk_build(p)),
                _ => None,
            })
        })
        .flatten();
    // Щупы по номерам флоатов (`lead-for`/`lead-base` = «a-b»).
    let probes_of = |base: bool| -> Vec<(usize, usize, crate::layout::float::band_flow::Build)> {
        nodes
            .iter()
            .filter_map(|n| match n {
                Node::Element(p) => {
                    let lead = p.marks.lead.filter(|l| l.base == base)?;
                    Some((lead.from, lead.to, mk_build(p)))
                }
                _ => None,
            })
            .collect()
    };
    let lead_probes = probes_of(false);
    let base_probes = probes_of(true);
    let base_for = |idx: usize| -> Option<crate::layout::float::band_flow::Build> {
        base_probes
            .iter()
            .find(|(a, b, _)| (*a..*b).contains(&idx))
            .map(|(_, _, b)| b.clone())
    };
    let lead_for = |idx: usize| -> Option<crate::layout::float::band_flow::Build> {
        if let Some(b) = nowrap_probe.as_ref() {
            return Some(b.clone());
        }
        lead_probes
            .iter()
            .find(|(a, b, _)| (*a..*b).contains(&idx))
            .map(|(_, _, b)| b.clone())
    };
    for (idx, n) in nodes.iter().enumerate() {
        let Node::Element(c) = n else {
            continue;
        };
        if c.marks.lead.is_some() {
            continue;
        }
        let mut continued;
        let c = if initial_letter && c.marks.anon {
            // The initial letter already consumed the paragraph's indent.
            continued = c.clone();
            continued.style.cascaded_mut().text_indent =
                Some(crate::style::values::value::Len::Px(0.0));
            &continued
        } else {
            c
        };
        let Some(margin) = band_margins(c.style.cascaded(), em) else {
            continue;
        };
        let [t, r, b, l] = margin;
        let margin = match vert {
            None => margin,
            Some(true) => [r, b, l, t],
            Some(false) if wm.sideways_lr() => [l, t, r, b],
            Some(false) => [l, b, r, t],
        };
        // Распорка держит физическую высоту — в вертикальном письме это не
        // блочный размер.
        if vert.is_some() && band_piece_m(n, em, wm) == Some(false) {
            continue;
        }
        let float = idx < count || c.style.cascaded().float.is_some_and(|f| f != 0);
        // `::first-line` содержащего блока — первой строке его потока
        // (CSS 2.1 §5.12.1); флоаты строк не образуют. Анонимный прогон
        // своего `first_line` не несёт (`element` берёт псевдоэлементы только
        // у самого узла), и первая строка хоста теряла свой кегль
        // (`below-float3`: `::first-line { font-size: 50px }` у `x` под
        // флоатом). Первый прогон получает слой хоста.
        let first_line = !float && !seen_inflow && c.marks.anon && !c.marks.cont;
        if !float && band_piece_m(n, em, wm) != Some(false) {
            seen_inflow = true;
        }
        let first_layer: Option<Computed> = if first_line {
            inherited.first_line.as_deref().cloned()
        } else {
            None
        };
        let mut nest: Option<Nest> = None;
        let kind = match band_kid_kind(inherited, opts, em, wm, vert, n, c, float, &mut nest) {
            Some(value) => value,
            None => continue,
        };
        let mut node = c.clone();
        if first_layer.is_some() {
            node.first_line = first_layer.map(Into::into);
        }
        let inherited = inherited.clone();
        let pagination_offset = opts.frag.float_struts.get(&c.node_id).map_or(0.0, |s| s.by);
        let shape_values = opts.cx.vcx().keep();
        let is_nest = nest.is_some();
        let vertical = vert.is_some();
        let cb_height = inherited.height;
        let cb_block_w = inherited.width;
        let build = deferred(opts, !is_nest, move |opts, cb, avail, shapes, height| {
            build_band_kid(
                float, &node, &inherited, opts, is_nest, vertical, cb_height, cb_block_w, cb,
                avail, shapes, height,
            )
        });
        let clear = if float {
            None
        } else {
            c.style.cascaded().clear
        };
        // css-shapes-1 §2: the float's shape shortens the lines next to it.
        let shape: Option<crate::layout::float::band_flow::ShapeOf> =
            (float && c.style.cascaded().shape_outside.is_some()).then(|| {
                let f = c.clone();
                let side = c.style.cascaded().float.unwrap_or(-1);
                let em = band_em(c.style.cascaded(), em).unwrap_or(em);
                std::rc::Rc::new(move |bw: f32, bh: f32, m: [f32; 4], cbw: f32| {
                    measured_shape(&f, side, (bw, bh), m, cbw, em, wm, shape_values.vcx())
                }) as crate::layout::float::band_flow::ShapeOf
            });
        kids.push(Kid {
            kind,
            node_id: c.node_id,
            pagination_offset,
            monolithic: crate::layout::fragment::clone::monolith_box(c),
            clear,
            margin,
            build,
            nest,
            anon: c.marks.anon,
            lead: if float { lead_for(idx) } else { None },
            lead_base: if float { base_for(idx) } else { None },
            margin_offset: c.style.cascaded().float_margin_offset.unwrap_or(0.0),
            start_open,
            shape,
        });
    }
    kids
}
