//! Сборка абзаца: вертикальный прямостоящий текст разных кеглей, выровненный
//! по центральной базовой линии.
//!
//! css-writing-modes-4 §4.2: при `text-orientation: upright` доминирующая
//! базовая — `central`; каждая строчная коробка центрирует свою высоту строки
//! на своей центральной базовой (css-inline-3 §4.2; Blink `inline_box_state.cc`
//! берёт центральную базовую для стойки и для каждой коробки). Каждый
//! прямостоящий глиф продвигается на свой em (§7.4), а не на em контейнера.
//! Возвращает `None`, когда простая стопка уже точна или содержимое не текст.

use crate::dom::Node;
use crate::layout::options::RenderOpts;
use crate::render::paragraph::{own_size, paragraph};
use crate::style::computed::{Align, BoxStyle, Computed, TextAlign};
use crate::style::values::value::Len;
use crate::tolerance::TEXT_COORD_EPS;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

struct Glyph {
    text: String,
    style: BoxStyle,
    size: f32,
    shift: f32,
}

/// Inline boxes (`shift` toward line-over, half line-height) and glyphs.
#[derive(Default)]
struct Stack {
    boxes: Vec<(f32, f32)>,
    glyphs: Vec<Glyph>,
}

fn line_height(style: &Computed, size: f32) -> f32 {
    match style.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * size,
        _ => size,
    }
}

/// A plain span: no box decoration in either axis and a vertical-align that
/// is `baseline` or `middle`.
fn plain_span(own: &Computed, declared: &Computed) -> bool {
    let none = crate::style::computed::Sides::default();
    own.background.is_none()
        && own.bg_image.is_none()
        && own.padding == none
        && own.margin == none
        && own.borders() == none
        && own.underline.is_none()
        && own.decors.is_empty()
        && declared.text_shadow.is_none()
        && own.position.is_none()
        && own.vertical_shift.is_none()
        && own.vertical_shift_pct.is_none()
        && own.vertical_shift_px.is_none()
        && own.vertical_shift_len.is_none()
        && own.vertical_align_text.is_none()
        && matches!(
            own.vertical_align,
            None | Some(Align::Baseline) | Some(Align::Center)
        )
}

fn collect(
    nodes: &[Node],
    style: &BoxStyle,
    shift: f32,
    opts: &RenderOpts,
    out: &mut Stack,
) -> Option<()> {
    let vcx = opts.cx.vcx();
    for node in nodes {
        match node {
            Node::Text(t) if t.trim().is_empty() => {}
            Node::Text(t) => {
                if t.trim().chars().any(char::is_whitespace) {
                    return None;
                }
                let size = own_size(style, opts);
                for ch in t.trim().chars() {
                    out.glyphs.push(Glyph {
                        text: ch.to_string(),
                        style: style.clone(),
                        size,
                        shift,
                    });
                }
            }
            Node::Element(e) => {
                if !crate::layout::box_kind::real_inline(e)
                    || !plain_span(&e.style.used(), e.style.cascaded())
                {
                    return None;
                }
                let child = crate::style::cascade::materialize::box_style(style, &e.style, vcx);
                let size = own_size(&child, opts);
                // `middle`: the box midpoint — its central baseline here —
                // sits on the parent's central baseline raised by half the
                // parent's x-height (CSS 2.1 §10.8.1; Blink kMiddle).
                let shift = if e.style.own_vertical_align() == Some(Align::Center) {
                    let family = style.font_family.clone().unwrap_or_default();
                    let parent = own_size(style, opts);
                    shift + crate::text::metrics::ch_ex_px(&family, parent, vcx).1 / 2.0
                } else {
                    shift
                };
                out.boxes.push((shift, line_height(&child, size) / 2.0));
                collect(&e.children, &child, shift, opts, out)?;
            }
        }
    }
    Some(())
}

/// The central-baseline stack, or `None` when the plain upright stack is
/// already exact (one size, no shifts) or the content is not plain text.
pub(super) fn upright_central(
    nodes: &[Node],
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    if inherited.wm().rtl() {
        return None;
    }
    let em = own_size(inherited, opts);
    let mut stack = Stack::default();
    stack.boxes.push((0.0, line_height(inherited, em) / 2.0));
    collect(nodes, inherited, 0.0, opts, &mut stack)?;
    let mixed = stack
        .glyphs
        .iter()
        .any(|g| (g.size - em).abs() > TEXT_COORD_EPS || g.shift.abs() > TEXT_COORD_EPS);
    if !mixed {
        return None;
    }
    let over = stack.boxes.iter().fold(0.0f32, |m, (s, h)| m.max(s + h));
    let under = stack.boxes.iter().fold(0.0f32, |m, (s, h)| m.max(h - s));
    let lane = over + under;
    let items = stack.glyphs.into_iter().map(|g| {
        let mut style = g.style;
        style.flat_mode = style.flow_mode();
        style.writing_mode = None;
        style.upright_stack = true;
        style.line_height = Some(Len::Px(g.size));
        style.text_align = Some(TextAlign::Center);
        let wide = 2.0 * lane.max(g.size);
        // Line-over is the physical right in both vertical modes.
        let left = under + g.shift - wide / 2.0;
        div().h(px(g.size)).w(px(lane)).relative().child(
            div()
                .absolute()
                .top_0()
                .left(px(left))
                .w(px(wide))
                .child(paragraph(&[Node::Text(g.text)], &style, opts)),
        )
    });
    Some(
        div()
            .flex()
            .flex_col()
            .w(px(lane))
            .flex_shrink_0()
            .children(items)
            .into_any_element(),
    )
}
