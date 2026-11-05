//! `<svg>` с `<foreignObject>`: внутри — независимый контекст форматирования HTML в порядке
//! покраски SVG. Каждый `foreignObject` раскладывается и снимается в PNG отдельным слоем, слой
//! подставляется в дерево как `<image>`, и весь рисунок растрируется как обычный `<svg>`;
//! единственный вклад с `mix-blend-mode` смешивается с подложкой хоста.
mod fingerprint;
mod geometry;
mod host;
mod layer;
use crate::dom::{Element, Node};
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

pub(super) fn contains(e: &Element) -> bool {
    let mut pending = vec![e];
    while let Some(e) = pending.pop() {
        if e.tag.eq_ignore_ascii_case("foreignObject") {
            return true;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        }));
    }
    false
}

pub(super) fn element(e: &Element, opts: &RenderOpts) -> Option<AnyElement> {
    let frame = opts.cx.frame();
    let values = frame.values.keep();
    let (w, h) = crate::layout::replaced::svg_size::size_of(e);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let tree = e.clone();
    let opts = opts.clone();
    let mut wrapper = gpui::div().w(px(w)).h(px(h)).flex_shrink_0();
    if let Some(bg) = e.style.used().reset_background.background {
        wrapper = wrapper.bg(bg.to_hsla());
    }
    let blend = geometry::sole_blend(e);
    let element = wrapper
        .child(
            host::Foreign {
                tree: Some(tree),
                opts,
                values,
                size: (w, h),
            }
            .hooked(),
        )
        .into_any_element();
    let style = crate::style::computed::Computed {
        reset_effects: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetEffects {
                blend,
                ..Default::default()
            },
        ),
        ..Default::default()
    };
    Some(crate::paint::effects::grouped::grouped(
        element,
        &crate::style::computed::BoxStyle::new(style),
        frame,
    ))
}
