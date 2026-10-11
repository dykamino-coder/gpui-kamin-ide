//! Части ребёнка полос: вид (флоат, кусок, поток, вложение) и заголовок первой строки.

use super::super::band_flow_block;
use crate::dom::{Element, Node};
use crate::layout::float::band_flow::Kind;
use crate::layout::float::band_host::px_margin_box;
use crate::layout::float::band_measured::band_piece_m;
use crate::layout::float::band_nest::{band_nest, cont_indent};
use crate::paint::effects::paint_scope::DepthScope;
use crate::render::{RenderOpts, element, is_blank};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::AnyElement;

#[allow(clippy::too_many_arguments)]
pub(crate) fn band_kid_kind(
    inherited: &Computed,
    opts: &RenderOpts,
    em: f32,
    vert: Option<bool>,
    n: &Node,
    c: &Element,
    float: bool,
    nest: &mut Option<super::super::super::band_flow::Nest>,
) -> Option<super::super::super::band_flow::Kind> {
    let kind = if float {
        Kind::Float {
            side: c.style.float.unwrap_or(-1),
            clear: c.style.clear,
            // Строчный размер `auto` — shrink-to-fit; в вертикальном
            // письме строчный размер — высота.
            shrink: if vert.is_some() {
                matches!(c.style.height, None | Some(Len::Auto))
            } else {
                matches!(c.style.width, None | Some(Len::Auto))
            },
            letter: c.attr("initial-letter") == Some("1"),
        }
    } else {
        match band_piece_m(n, em) {
            Some(true) => Kind::Piece {
                rtl: inherited.rtl == Some(true),
                table: c.tag == "table" || matches!(c.style.display, Some(Display::Table)),
            },
            Some(false) => Kind::Strut(px_margin_box(&c.style).map_or(0.0, |(_, h)| h)),
            None if c.attr("anon") == Some("1") || band_flow_block(c, em) => Kind::Flow,
            None => match band_nest(c, inherited, opts, em) {
                Some(nn) => {
                    *nest = Some(nn);
                    Kind::Nest
                }
                None => return None,
            },
        }
    };
    Some(kind)
}

pub(crate) fn band_kid_head(
    inherited: &Computed,
    opts: &RenderOpts,
    depth: crate::paint::effects::paint_scope::Depth,
    c: &Element,
    first_layer: &Option<Computed>,
) -> Option<
    std::rc::Rc<
        dyn Fn(
                f32,
                f32,
                Option<
                    std::sync::Arc<(
                        Vec<super::super::super::shapes::FloatShape>,
                        Vec<super::super::super::shapes::FloatShape>,
                    )>,
                >,
                Option<f32>,
            ) -> AnyElement
            + 'static,
    >,
> {
    let head: Option<crate::layout::float::band_flow::Build> = if c.attr("anon") == Some("1")
        && inherited.nowrap != Some(true)
        && inherited.keep_spaces != Some(true)
    {
        match c.children.iter().find(|n| !is_blank(n)) {
            Some(Node::Text(t)) => t.split_whitespace().next().map(|word| {
                let mut hn = c.clone();
                cont_indent(&mut hn, inherited);
                if first_layer.is_some() {
                    hn.first_line = first_layer.clone();
                }
                hn.children = vec![Node::Text(word.to_string())];
                let inherited = inherited.clone();
                let opts = opts.clone();
                let b: crate::layout::float::band_flow::Build =
                    std::rc::Rc::new(move |_cb: f32, _avail: f32, _shapes, _h: Option<f32>| {
                        let _depth = DepthScope::enter(depth);
                        element(&hn, &inherited, &opts)
                    });
                b
            }),
            _ => None,
        }
    } else {
        None
    };
    head
}
