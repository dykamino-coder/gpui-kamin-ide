//! Вложенные полосы обтекания.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::block::margins::collapse_margins;
use crate::layout::block::struts::top_edge_open;
use crate::layout::float::band_clearance::supported as band_clear_supported;
use crate::layout::float::band_flow_host::{
    band_flow_rest, band_kids, band_orthogonal, trimmed_clear,
};
use crate::layout::float::band_host::{band_edge, band_em, band_margins};
use crate::layout::float::band_measured::{band_float_m, lead_probes};
use crate::render::{RenderOpts, block_level_in_flow, is_blank, own_context, replaced_tag};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::writing_mode::WritingMode;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Прогон-продолжение после `<br>` (`cont`, `band_flow_rest`): его первая
/// строка — не первая строка абзаца, и `text-indent` её не сдвигает
/// (css-text-3 §8.1), кроме `each-line` (сдвигает и строку после
/// принудительного разрыва) и `hanging` (сдвигает все, кроме первой) — там
/// отступ остаётся унаследованным.
pub(super) fn cont_indent(e: &mut Element, inherited: &Computed) {
    if e.marks.cont
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
pub(super) fn band_nest_block(c: &Element, em: f32, wm: WritingMode) -> bool {
    // Рамка, поля и высота коробки `Kind::Nest` — физические.
    !wm.vertical()
        // A box whose writing mode differs from its parent's establishes an
        // independent formatting context (css-writing-modes-4 §3.2): the
        // host's floats do not reach into it.
        && !band_orthogonal(&c.style, wm)
        && !c.children.iter().any(|k| matches!(k, Node::Element(k) if trimmed_clear(c, k)))
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
            || (band_em(&c.style, em).and_then(|em| band_edge(&c.style.width, em)).is_some()
                && c.style.border_box != Some(true)))
        && matches!(c.style.height, None | Some(Len::Auto) | Some(Len::Px(_) | Len::Pct(_)))
        && c.style.min_height.is_none()
        && c.style.max_height.is_none()
        && band_margins(&c.style, em).is_some()
        && band_inset(c, em).is_some()
}

/// Рамка плюс отступ коробки (верх, право, низ, лево) для `Kind::Nest`.
fn band_inset(c: &Element, em: f32) -> Option<[crate::layout::float::band_flow::Edge; 4]> {
    use crate::layout::float::band_flow::Edge;
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
pub(super) fn band_nest(
    c: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    em: f32,
    wm: WritingMode,
) -> Option<crate::layout::float::band_flow::Nest> {
    if !band_nest_block(c, em, wm) {
        return None;
    }
    let vcx = opts.cx.vcx();
    let cb = opts.cx.cb_width;
    let inner_em = band_em(&c.style, em)?;
    let mut merged = inherit(inherited, &c.style, vcx);
    let height = crate::layout::block::available_height::inner(
        &merged,
        opts.cx.avail_h,
        opts.cx.avail_w,
        opts.viewport.1,
    );
    if let Some(height) = height {
        merged.height = Some(Len::Px(height));
    }
    let opts = &*opts.inner(|cx| cx.avail_h = height);
    crate::render::pseudo_line_layers::install(c, &mut merged, vcx);
    // The first formatted line may belong to a nested in-flow block;
    // floats and positioned children cannot consume its pseudo layer.
    let children = crate::render::first_line_descendants::route(
        collapse_margins(&c.children, false, cb),
        &merged,
    );
    let seq = band_seq(
        children,
        inner_em,
        wm,
        cb,
        merged.preserve_newlines == Some(true),
    )?;
    let kids = band_kids(&seq, 0, &merged, opts, inner_em, top_edge_open(c), wm);
    Some(crate::layout::float::band_flow::Nest {
        kids,
        inset: band_inset(c, em)?,
        height,
        width: if matches!(c.style.width, None | Some(Len::Auto)) {
            None
        } else {
            Some(band_edge(&c.style.width, inner_em)?)
        },
    })
}

/// Дети `Kind::Nest`: как хвост хоста (`band_flow_rest`), но флоаты стоят
/// между блоками на своём месте (правило 5/6 §9.5.1 — потолок от низа
/// предыдущего блока). Внутри абзаца тот же LeadProbe, что у хоста,
/// передаёт правилу 6 верх текущей строки, не разрывая сам абзац.
pub(super) fn band_seq(
    nodes: Vec<Node>,
    em: f32,
    wm: WritingMode,
    cb: Option<f32>,
    preserve: bool,
) -> Option<Vec<Node>> {
    let mut out: Vec<Node> = vec![];
    let mut chunk: Vec<Node> = vec![];
    let mut probes = vec![];
    for n in nodes {
        let float = matches!(&n, Node::Element(c) if c.style.float.is_some_and(|f| f != 0));
        if !float {
            chunk.push(n);
            continue;
        }
        let Node::Element(f) = &n else {
            continue;
        };
        // CSS 2.1 section 9.5: floats in nested flow blocks participate in
        // the same BFC. Match the measured host's orthogonal-float gate;
        // a text-free vertical float does not need inline text measurement.
        if (f.style.wm().vertical() && band_float_m(f, em, wm).is_none())
            || band_margins(&f.style, em).is_none()
        {
            return None;
        }
        // Flush completed blocks, retaining the inline prefix: the text on
        // either side of this float belongs to one anonymous paragraph.
        let end = chunk
            .iter()
            .rposition(|n| matches!(n, Node::Element(c) if block_level_in_flow(c)))
            .map_or(0, |i| i + 1);
        let line = chunk.split_off(end);
        out.extend(band_flow_rest(
            std::mem::replace(&mut chunk, line),
            em,
            wm,
            cb,
        )?);
        if chunk.iter().any(|n| !is_blank(n)) {
            probes.extend(lead_probes(
                chunk.clone(),
                out.len(),
                out.len() + 1,
                preserve,
            ));
        }
        out.push(n);
    }
    out.extend(band_flow_rest(chunk, em, wm, cb)?);
    out.extend(probes);
    Some(out)
}

#[cfg(test)]
mod tests;
