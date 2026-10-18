//! Коробка места для inset позиционированного элемента.

use crate::dom::Element;
use crate::render::*;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Len, px_lossy_zero};
use gpui::{AnyElement, IntoElement, ParentElement, div};

pub(crate) fn inset_holder_box(
    e: &Element,
    block_axis: bool,
    inherited: &Computed,
    opts: &RenderOpts,
    kw_len: &impl Fn(Option<Len>) -> bool,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let mut holder = Computed {
        position: e.style.position,
        inset: e.style.inset,
        z_index: e.style.z_index,
        display: Some(Display::Flex),
        flex_dir: Some(if block_axis {
            crate::style::computed::FlexDir::Col
        } else {
            crate::style::computed::FlexDir::Row
        }),
        ..Default::default()
    };
    // Поля вдоль оси остаются у ВНУТРЕННЕЙ коробки: auto-поля
    // элемента гибкого контейнера забирают остаток, а при нехватке
    // места обнуляются (css-flexbox-1 §8.1) — ровно как auto-поля
    // абсолюта (§3.8; `fit-content-block-size-abspos` с
    // переполнением). Поперечные не-auto поля — у держателя.
    // Поперечные поля — у держателя целиком, включая auto: с
    // заданным размером и краями с обеих сторон они центрируют
    // сам абсолют (css-position-3 §3.8; `inline-size: 100px;
    // margin: auto; inset: 0`).
    if block_axis {
        holder.margin.left = e.style.margin.left;
        holder.margin.right = e.style.margin.right;
    } else {
        holder.margin.top = e.style.margin.top;
        holder.margin.bottom = e.style.margin.bottom;
    }
    // Поперечный размер держателя — border-box внутренней коробки:
    // её рамка и отбивка прибавляются (у держателя своих нет).
    let px_of = px_lossy_zero;
    let bd = e.style.borders();
    if block_axis {
        let extra = px_of(e.style.padding.left)
            + px_of(e.style.padding.right)
            + px_of(bd.left)
            + px_of(bd.right);
        holder.width = match e.style.width {
            Some(Len::Px(w)) => Some(Len::Px(w + extra)),
            other => other.filter(|l| !kw_len(Some(*l))),
        };
    } else {
        let extra = px_of(e.style.padding.top)
            + px_of(e.style.padding.bottom)
            + px_of(bd.top)
            + px_of(bd.bottom);
        holder.height = match e.style.height {
            Some(Len::Px(h)) => Some(Len::Px(h + extra)),
            other => other.filter(|l| !kw_len(Some(*l))),
        };
    }
    let mut inner = e.clone();
    inner.style.position = None;
    inner.style.inset = Default::default();
    if block_axis {
        inner.style.margin.left = None;
        inner.style.margin.right = None;
    } else {
        inner.style.margin.top = None;
        inner.style.margin.bottom = None;
    }
    inner.style.z_index = None;
    crate::style::apply::apply(div(), &holder, vcx)
        .child(element(&inner, inherited, opts))
        .into_any_element()
}
