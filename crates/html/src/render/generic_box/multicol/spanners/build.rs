//! Сборка анонимного потока многоколоночника со спаннерами при разрешённой
//! ширине: план колонок и стопка рядов под хостом.

use super::super::super::columns::column_plan_in;
use super::super::settled;
use crate::dom::Element;
use crate::layout::fragment::token::space::sequence::Nested;
use crate::layout::multicol::dimensions::MulticolDimensions;
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, Styled, div, px};
use std::rc::Rc;

#[allow(clippy::too_many_arguments)]
pub(super) fn build<const QUERY: bool>(
    host: &Element,
    style: BoxStyle,
    inherited: &Computed,
    opts: &RenderOpts,
    outer: Option<Rc<Nested>>,
    rest: Option<f32>,
    (dimensions, own): (MulticolDimensions, MulticolDimensions),
) -> AnyElement {
    let p = column_plan_in(host, &style, opts, dimensions, own);
    let mut d = div();
    let vertical = style.wm().vertical();
    let axes = style.wm().reversed_axes();
    d.style().block_flow = Some([vertical, vertical && axes[0], axes[usize::from(vertical)]]);
    // Размер анонимного потока задаёт CSS-хост; декорации остаются снаружи.
    if let Some(Len::Px(h)) = dimensions.preferred[1] {
        d = d.h(px(h));
    }
    match settled::<QUERY>(
        d,
        host,
        style,
        inherited,
        opts,
        p.column_width,
        p.used_count.unwrap_or(0),
        p.used_gap,
        p.col_axis,
        p.col_vert,
        p.col_rl,
        p.col_inline_size,
        outer,
        p.lone_span,
        rest,
        Some((dimensions, own)),
        None,
    ) {
        Ok((d, _)) => d.into_any_element(),
        Err(el) => el,
    }
}
