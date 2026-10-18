//! Textarea natural rows/columns geometry, separate from its painted editor.

use super::{Muted, field_box};
use crate::document::state::ValueCx;
mod content;
use crate::style::computed::FontCx;
use crate::{dom::Element, style::computed::Computed, style::values::value::Len};
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div};

pub(super) fn textarea(
    e: &Element,
    style: &Computed,
    opts: &crate::render::RenderOpts,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let mut text = String::new();
    crate::render::gather_text_public(&e.children, &mut text);
    let body = if text.is_empty() {
        div()
            .when_muted(true, style)
            .child(SharedString::from(
                e.attr("placeholder").unwrap_or("").to_owned(),
            ))
            .into_any_element()
    } else {
        crate::render::paragraph_public(&e.children, style, opts)
    };
    let natural = natural(e, style, vcx);
    let mut paint = field_box(style, vcx)
        .items_start()
        .child(div().w_full().child(body));
    // UA dimensions are preferred intrinsic sizes, never authored minima.
    paint.style().min_size = std::mem::take(
        &mut crate::style::apply::apply(div(), style, vcx)
            .style()
            .min_size,
    );
    // The editor owns scroll state; an anonymous viewport would become the
    // flex item and lose the host's growth, basis and intrinsic constraints.
    paint = crate::interactive::scroll_target::attach(paint, e, style, vcx);
    let keywords = crate::style::apply::intrinsic_size::keywords(style)
        .map(|keyword| keyword.filter(|v| !matches!(v, gpui::CssSizingKeyword::Stretch)));
    paint.style().sizing_keywords = Some(keywords);
    let gutter = gutters(style);
    let nodes = if text.is_empty() {
        vec![crate::dom::Node::Text(
            e.attr("placeholder").unwrap_or("").to_owned(),
        )]
    } else {
        e.children.clone()
    };
    let measure = content::content_measure(nodes, style, opts, natural, gutter);
    crate::layout::replaced::natural_control_with(
        paint,
        gpui::size(gpui::px(natural[0]), gpui::px(natural[1])),
        measure,
    )
}

fn natural(e: &Element, c: &Computed, vcx: ValueCx<'_>) -> [f32; 2] {
    let font = match c.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.,
    };
    let family = c.font_family.as_deref().unwrap_or("");
    let (ch, _) = crate::text::metrics::ch_ex_px(family, font);
    let line = c
        .line_height
        .and_then(|v| v.to_px(None, FontCx::new(family, font), vcx))
        .unwrap_or_else(|| {
            crate::text::metrics::normal_px(crate::text::metrics::normal_line(family), font)
        });
    let vertical = c.wm().vertical();
    let gutter = gutters(c);
    let inline = (positive(e.attr("cols"), 20) as f32 * ch).ceil()
        + if vertical { gutter[1] } else { gutter[0] };
    let block =
        positive(e.attr("rows"), 2) as f32 * line + if vertical { gutter[0] } else { gutter[1] };
    if vertical {
        [block, inline]
    } else {
        [inline, block]
    }
}

fn gutters(c: &Computed) -> [f32; 2] {
    use crate::style::computed::Overflow;
    let reserve = |overflow| {
        if overflow == Some(Overflow::Scroll) {
            c.scroll.thickness()
        } else {
            0.
        }
    };
    // The block-axis auto scrollbar contributes to the inline natural size;
    // the inline-axis scrollbar contributes only when always requested.
    if c.wm().vertical() {
        [
            if c.overflow_y == Some(Overflow::Scroll) && c.scroll.forced[1] {
                c.scroll.thickness()
            } else {
                0.
            },
            reserve(c.overflow_x),
        ]
    } else {
        [
            reserve(c.overflow_y),
            if c.overflow_x == Some(Overflow::Scroll) && c.scroll.forced[0] {
                c.scroll.thickness()
            } else {
                0.
            },
        ]
    }
}

fn positive(value: Option<&str>, default: u32) -> u32 {
    value
        .and_then(|v| {
            let v = v.trim_start_matches([' ', '\t', '\n', '\r', '\u{c}']);
            let v = v.strip_prefix('+').unwrap_or(v);
            let digits = &v[..v.bytes().take_while(u8::is_ascii_digit).count()];
            digits.parse::<u32>().ok().filter(|v| *v > 0)
        })
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::positive;

    #[test]
    fn html_positive_integer_prefixes_and_defaults() {
        assert_eq!(positive(Some(" +12cols"), 20), 12);
        for value in [None, Some("0"), Some("-2"), Some(""), Some("NaN")] {
            assert_eq!(positive(value, 20), 20);
        }
        assert_eq!(positive(Some("3.5"), 2), 3);
    }
}
