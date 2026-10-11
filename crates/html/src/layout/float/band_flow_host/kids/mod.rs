//! Дети полос из узлов потока (band_kids): вид, поля, заголовок первой строки.

use super::build_band_kid;
use crate::dom::{Element, Node};
use crate::layout::float::band_host::band_margins;
use crate::layout::float::band_measured::band_piece_m;
use crate::paint::effects::paint_scope::DepthScope;
use crate::paint::effects::paint_scope::snapshot as defer_depth;
use crate::render::{RenderOpts, element};
use crate::style::computed::Computed;
mod parts;
pub(super) use parts::band_kid_head;
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
) -> Vec<crate::layout::float::band_flow::Kid> {
    use crate::layout::float::band_flow::{Kid, Nest};
    let depth = defer_depth();
    let mut kids: Vec<Kid> = vec![];
    // Письмо содержащего блока: план хоста — в логических осях, поля
    // переводятся в (block-start, inline-end, block-end, inline-start)
    // по таблице css-writing-modes-4 :1877-1888 (`vertical-rl`: block-start
    // — право, inline-start — верх; `vertical-lr`: block-start — лево).
    let vert = (inherited.vertical == Some(true)).then_some(inherited.vertical_rl == Some(true));
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
        .any(|n| matches!(n, Node::Element(p) if p.attr("lead-probe") == Some("1")));
    let nowrap = inherited.nowrap == Some(true);
    let mk_build = |p: &Element| -> crate::layout::float::band_flow::Build {
        let p = p.clone();
        let inherited = inherited.clone();
        let opts = opts.clone();
        std::rc::Rc::new(move |_cb: f32, _avail: f32, _shapes, _h: Option<f32>| {
            let _depth = DepthScope::enter(depth);
            element(&p, &inherited, &opts)
        })
    };
    // Первый прогон — щуп для всех флоатов при `nowrap`.
    let nowrap_probe: Option<crate::layout::float::band_flow::Build> = (nowrap && has_lead)
        .then(|| {
            nodes.iter().find_map(|n| match n {
                Node::Element(p)
                    if p.attr("anon") == Some("1") && p.attr("lead-probe").is_none() =>
                {
                    Some(mk_build(p))
                }
                _ => None,
            })
        })
        .flatten();
    // Щупы по номерам флоатов (`lead-for`/`lead-base` = «a-b»).
    let probes_of = |key: &str| -> Vec<(usize, usize, crate::layout::float::band_flow::Build)> {
        nodes
            .iter()
            .filter_map(|n| match n {
                Node::Element(p) if p.attr("lead-probe") == Some("1") => {
                    let (a, b) = p.attr(key)?.split_once('-')?;
                    Some((a.parse().ok()?, b.parse().ok()?, mk_build(p)))
                }
                _ => None,
            })
            .collect()
    };
    let lead_probes = probes_of("lead-for");
    let base_probes = probes_of("lead-base");
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
        if c.attr("lead-probe") == Some("1") {
            continue;
        }
        let Some(margin) = band_margins(&c.style, em) else {
            continue;
        };
        let [t, r, b, l] = margin;
        let margin = match vert {
            None => margin,
            Some(true) => [r, b, l, t],
            Some(false) => [l, b, r, t],
        };
        // Распорка держит физическую высоту — в вертикальном письме это не
        // блочный размер.
        if vert.is_some() && band_piece_m(n, em) == Some(false) {
            continue;
        }
        let float = idx < count || c.style.float.is_some_and(|f| f != 0);
        // `::first-line` содержащего блока — первой строке его потока
        // (CSS 2.1 §5.12.1); флоаты строк не образуют. Анонимный прогон
        // своего `first_line` не несёт (`element` берёт псевдоэлементы только
        // у самого узла), и первая строка хоста теряла свой кегль
        // (`below-float3`: `::first-line { font-size: 50px }` у `x` под
        // флоатом). Первый прогон получает слой хоста.
        let first_line =
            !float && !seen_inflow && c.attr("anon") == Some("1") && c.attr("cont").is_none();
        if !float && band_piece_m(n, em) != Some(false) {
            seen_inflow = true;
        }
        let first_layer: Option<Computed> = if first_line {
            inherited.first_line.as_deref().cloned()
        } else {
            None
        };
        let mut nest: Option<Nest> = None;
        let kind = match band_kid_kind(inherited, opts, em, vert, n, c, float, &mut nest) {
            Some(value) => value,
            None => continue,
        };
        // Голова анонимного прогона — его первое слово (до первой мягкой
        // возможности разрыва): по ней план решает, влезает ли ПЕРВАЯ строка
        // в окно рядом с флоатами (§9.5). min-content всего прогона — самое
        // длинное слово где-то дальше — опускал прогон под флоат, хотя
        // первые строки рядом помещались (`shape-image-012-ref`: флоат 100px
        // в 200px, строки `XXXXX` по 100px рядом, а `XXXXXXXXXX` — ниже).
        // При `nowrap` и значимых пробелах слово не граница строки — голову
        // не заводим, мерится весь прогон.
        let head = band_kid_head(inherited, opts, depth, c, &first_layer);
        let mut node = c.clone();
        if first_layer.is_some() {
            node.first_line = first_layer;
        }
        let inherited = inherited.clone();
        let opts = opts.clone();
        let is_nest = nest.is_some();
        let vertical = vert.is_some();
        let cb_height = inherited.height;
        let cb_block_w = inherited.width;
        let build: crate::layout::float::band_flow::Build =
            std::rc::Rc::new(move |cb: f32, avail: f32, shapes, height: Option<f32>| {
                build_band_kid(
                    depth, float, &node, &inherited, &opts, is_nest, vertical, cb_height,
                    cb_block_w, cb, avail, shapes, height,
                )
            });
        let clear = if float { None } else { c.style.clear };
        kids.push(Kid {
            kind,
            clear,
            margin,
            build,
            nest,
            anon: c.attr("anon") == Some("1"),
            head,
            lead: if float { lead_for(idx) } else { None },
            lead_base: if float { base_for(idx) } else { None },
            margin_offset: c.style.float_margin_offset.unwrap_or(0.0),
            start_open,
        });
    }
    kids
}
