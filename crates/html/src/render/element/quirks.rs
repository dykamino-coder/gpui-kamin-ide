//! Quirks Mode §§3.6–3.7: минимальные используемые блочные размеры корня документа и body.

use crate::dom::Element;
use crate::render::RenderOpts;
use crate::style::computed::{BoxStyle, Computed, Display, Position};
use crate::style::values::value::Len;

pub(super) fn fill(e: &Element, st: &mut Computed, parent: &BoxStyle, opts: &RenderOpts) {
    if !e.marks.quirks_fill
        || matches!(st.display, Some(Display::None | Display::Contents))
        || matches!(st.position, Some(Position::Absolute | Position::Fixed))
        || st.float.is_some_and(|f| f != 0)
        || crate::render::inline_level(e)
        || crate::render::inline_level_box(e)
        || st.column_span == Some(true)
    {
        return;
    }
    let vertical = st.wm().vertical();
    let size = if vertical { st.width } else { st.height };
    if !matches!(size, None | Some(Len::Auto)) || st.calc_size[usize::from(!vertical)].is_some() {
        return;
    }
    let vcx = opts.cx.vcx();
    let basis = if parent.wm().vertical() {
        parent.inline_definite
    } else {
        opts.cx.constraints.avail_w
    };
    let px = |value: Option<Len>, style: &Computed| match value {
        None | Some(Len::Auto) => 0.0,
        Some(v) => v.to_px(basis, style.font_cx(vcx), vcx).unwrap_or(0.0),
    };
    let edges = |style: &Computed| {
        let b = style.borders();
        if vertical {
            px(b.left, style)
                + px(b.right, style)
                + px(style.padding.left, style)
                + px(style.padding.right, style)
        } else {
            px(b.top, style)
                + px(b.bottom, style)
                + px(style.padding.top, style)
                + px(style.padding.bottom, style)
        }
    };
    let margins = |style: &Computed| {
        if vertical {
            px(style.margin.left, style) + px(style.margin.right, style)
        } else {
            px(style.margin.top, style) + px(style.margin.bottom, style)
        }
    };
    let viewport = if vertical {
        opts.viewport.0
    } else {
        opts.viewport.1
    };
    let available = if st.root_box {
        viewport
    } else {
        let parent_size = if vertical {
            parent.width
        } else {
            parent.height
        };
        let parent_min = if vertical {
            parent.min_width
        } else {
            parent.min_height
        };
        match parent_size {
            _ if !vertical && opts.cx.constraints.avail_h.is_some() => {
                opts.cx.constraints.avail_h.unwrap_or(0.0)
            }
            Some(v) if v != Len::Auto => {
                let size = v
                    .to_px(Some(viewport), parent.font_cx(vcx), vcx)
                    .unwrap_or(0.0);
                size - if parent.border_box == Some(true) {
                    edges(parent)
                } else {
                    0.0
                }
            }
            _ => {
                px(parent_min, parent)
                    - if parent.border_box == Some(true) {
                        edges(parent)
                    } else {
                        0.0
                    }
            }
        }
    };
    let minimum = (available
        - margins(st)
        - if st.border_box == Some(true) {
            0.0
        } else {
            edges(st)
        })
    .max(0.0);
    let old = if vertical {
        st.min_width
    } else {
        st.min_height
    };
    let old = old
        .and_then(|v| v.to_px(Some(available), st.font_cx(vcx), vcx))
        .unwrap_or(0.0);
    let minimum = Len::Px(minimum.max(old));
    if vertical {
        st.min_width = Some(minimum);
    } else {
        st.min_height = Some(minimum);
    }
}

#[cfg(test)]
mod tests;
