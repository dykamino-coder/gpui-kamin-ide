//! Части ребёнка полос: вид (флоат, кусок, поток, вложение) и заголовок первой строки.

use super::super::band_flow_block;
use crate::dom::{Element, Node};
use crate::layout::float::band_flow::Kind;
use crate::layout::float::band_host::px_margin_box;
use crate::layout::float::band_measured::band_piece_m;
use crate::layout::float::band_nest::band_nest;
use crate::render::RenderOpts;
use crate::style::computed::writing_mode::WritingMode;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn band_kid_kind(
    inherited: &Computed,
    opts: &RenderOpts,
    em: f32,
    wm: WritingMode,
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
            letter: c.marks.initial_letter,
            own_layout: super::super::own_layout_mode(&c.style),
        }
    } else {
        match band_piece_m(n, em, wm) {
            Some(true) => Kind::Piece {
                rtl: inherited.wm().rtl(),
                table: c.tag == "table" || matches!(c.style.display, Some(Display::Table)),
            },
            Some(false) => Kind::Strut(px_margin_box(&c.style).map_or(0.0, |(_, h)| h)),
            None if c.marks.anon || band_flow_block(c, em) => Kind::Flow,
            None => match band_nest(c, inherited, opts, em, wm) {
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
