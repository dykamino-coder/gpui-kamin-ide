//! Вложенные полосы обтекания.
// owner: A

use crate::style::cascade::inherit::inherit;
use crate::dom::{Element, Node};
use crate::layout::block::margins::collapse_margins;
use crate::layout::block::struts::top_edge_open;
use crate::layout::float::band_clearance::supported as band_clear_supported;
use crate::layout::float::band_dimensions;
use crate::layout::float::band_flow_host::{BAND_WM, band_flow_rest, band_kids};
use crate::layout::float::band_host::{band_edge, band_em, band_margins};
use crate::layout::float::band_measured::band_float_m;
use crate::render::{RenderOpts, block_level_in_flow, is_blank, out_of_flow, own_context, replaced_tag};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Прогон-продолжение после `<br>` (`cont`, `band_flow_rest`): его первая
/// строка — не первая строка абзаца, и `text-indent` её не сдвигает
/// (css-text-3 §8.1), кроме `each-line` (сдвигает и строку после
/// принудительного разрыва) и `hanging` (сдвигает все, кроме первой) — там
/// отступ остаётся унаследованным.
pub(crate) fn cont_indent(e: &mut Element, inherited: &Computed) {
    if e.attr("cont") == Some("1")
        && inherited.text_indent_each_line != Some(true)
        && inherited.text_indent_hanging != Some(true)
    {
        e.style.text_indent = Some(Len::Px(0.0));
    }
}

/// Можно ли разложить блок потока детьми на ОБЩИХ полосах (шаг F7): тот же
/// гейт, что у блока потока (`band_flow_block`), кроме чистоты содержимого,
/// плюс то, что коробка рисуется отдельно от детей — значит ни сдвига, ни
/// эффектов группы, ни ограничителей высоты; рамка и отступ разрешимы.
pub(crate) fn band_nest_block(c: &Element, em: f32) -> bool {
    // Рамка, поля и высота коробки `Kind::Nest` — физические.
    BAND_WM.with(std::cell::Cell::get) == 0
        && block_level_in_flow(c)
        && !own_context(c)
        && !replaced_tag(c)
        && c.style.is_caption != Some(true)
        && c.tag != "caption"
        && matches!(c.style.display, None | Some(Display::Block))
        && (c.style.clear.is_none() || band_clear_supported(c))
        && c.style.position.is_none()
        && c.style.transform.is_none()
        && c.style.opacity.is_none()
        && c.style.filter.is_none()
        && (matches!(c.style.width, None | Some(Len::Auto))
            || (band_dimensions::content_width(&c.style, em).is_some()
                && c.style.border_box != Some(true)))
        && matches!(c.style.height, None | Some(Len::Auto) | Some(Len::Px(_)))
        && c.style.min_height.is_none()
        && c.style.max_height.is_none()
        && band_margins(&c.style, em).is_some()
        && band_inset(c, em).is_some()
}

/// Рамка плюс отступ коробки (верх, право, низ, лево) для `Kind::Nest`.
pub(crate) fn band_inset(c: &Element, em: f32) -> Option<[crate::band_flow::Edge; 4]> {
    use crate::band_flow::Edge;
    let em = band_em(&c.style, em)?;
    let b = c.style.borders();
    let side = |p: &Option<Len>, b: &Option<Len>| -> Option<Edge> {
        let pe = band_edge(p, em)?;
        let be = match band_edge(b, em)? {
            Edge::Px(v) => v,
            // Рамка долей не бывает (css-backgrounds-3 §4.3).
            Edge::Pct(_) => return None,
        };
        Some(match pe {
            Edge::Px(v) => Edge::Px(v + be),
            Edge::Pct(k) if be == 0.0 => Edge::Pct(k),
            Edge::Pct(_) => return None,
        })
    };
    Some([
        side(&c.style.padding.top, &b.top)?,
        side(&c.style.padding.right, &b.right)?,
        side(&c.style.padding.bottom, &b.bottom)?,
        side(&c.style.padding.left, &b.left)?,
    ])
}

/// Содержимое `Kind::Nest`: дети блока, разложенные тем же разбором, что
/// хвост хоста (`band_seq`), со своим наследованием и кеглем.
pub(crate) fn band_nest(
    c: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    em: f32,
) -> Option<crate::band_flow::Nest> {
    if !band_nest_block(c, em) {
        return None;
    }
    let inner_em = band_em(&c.style, em)?;
    let seq = band_seq(collapse_margins(&c.children, false), inner_em)?;
    let merged = inherit(inherited, &c.style);
    let kids = band_kids(&seq, 0, &merged, opts, inner_em, top_edge_open(c));
    Some(crate::band_flow::Nest {
        kids,
        inset: band_inset(c, em)?,
        height: match c.style.height {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        },
        width: band_dimensions::content_width(&c.style, em),
    })
}

/// Дети `Kind::Nest`: как хвост хоста (`band_flow_rest`), но флоаты стоят
/// между блоками на своём месте (правило 5/6 §9.5.1 — потолок от низа
/// предыдущего блока). Флоат посреди строчного прогона (правило 6 со
/// «верхом текущей строки») — не наш случай, отказ.
pub(crate) fn band_seq(nodes: Vec<Node>, em: f32) -> Option<Vec<Node>> {
    let mut out: Vec<Node> = vec![];
    let mut chunk: Vec<Node> = vec![];
    for n in nodes {
        let float = matches!(&n, Node::Element(c) if c.style.float.is_some_and(|f| f != 0));
        if !float {
            chunk.push(n);
            continue;
        }
        let Node::Element(f) = &n else {
            continue;
        };
        // Строчное содержимое ДО флоата и ПОСЛЕ — один прогон: флоат
        // посреди строки, отказ.
        let open_run = chunk
            .iter()
            .rev()
            .find(|m| !is_blank(m))
            .is_some_and(|m| match m {
                Node::Text(_) => true,
                Node::Element(e) => !block_level_in_flow(e) && !out_of_flow(&e.style),
            });
        // CSS 2.1 section 9.5: floats in nested flow blocks participate in
        // the same BFC. Match the measured host's orthogonal-float gate;
        // a text-free vertical float does not need inline text measurement.
        if open_run
            || (f.style.vertical == Some(true) && band_float_m(f, em).is_none())
            || f.style.shape_outside.is_some()
            || band_margins(&f.style, em).is_none()
        {
            return None;
        }
        out.extend(band_flow_rest(std::mem::take(&mut chunk), em)?);
        out.push(n);
    }
    out.extend(band_flow_rest(chunk, em)?);
    Some(out)
}
