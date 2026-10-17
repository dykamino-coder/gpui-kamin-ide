//! MathML Core formatting contexts, separate from HTML block/inline layout.

mod arrangement;
mod assembly;
mod content;
mod font_parameters;
mod fraction;
mod geometry;
mod glyph;
mod limits;
mod multiscripts;
mod operators;
mod padded;
mod parameters;
mod radical;
mod scripts;
mod stretch;

use crate::dom::{Element, Node};
use crate::render::RenderOpts;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use content::Content;
use geometry::Metrics;
use gpui::{AnyElement, IntoElement, ParentElement, Styled};
use parameters::{Kind, length_px, px_value};

pub(crate) fn element(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    build(e, inherited, opts, None)
}
fn build(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    target: Option<stretch::Target>,
) -> Option<AnyElement> {
    if !e.style.mathml || e.style.math_layout.is_none() {
        return None;
    }
    let mut style = inherit(inherited, &e.style);
    style.resolve_viewport(opts.viewport);
    let token = matches!(
        e.tag.as_str(),
        "mi" | "mn" | "mo" | "ms" | "mtext" | "annotation" | "annotation-xml"
    );
    let mut factories = Vec::new();
    let mut operator = None;
    let (children, kind) = if e.tag == "mspace" {
        let ascent = e
            .attr("height")
            .and_then(|v| length_px(v, &style, opts))
            .unwrap_or(0.0)
            .max(0.0);
        (
            vec![],
            Kind::Space(Metrics {
                width: px_value(style.width).unwrap_or(0.0).max(0.0),
                ascent,
                descent: px_value(style.height).unwrap_or(0.0).max(0.0) - ascent,
            }),
        )
    } else if token {
        if e.tag == "mo" {
            operator = operators::text(e).chars().next().zip(target);
        }
        (
            vec![crate::render::paragraph::paragraph(
                &e.children,
                &style,
                opts,
            )],
            if e.tag == "mo" {
                let op = operators::operator(e, &style, opts);
                Kind::Operator(op.left, op.right)
            } else {
                Kind::Row
            },
        )
    } else {
        let children =
            e.children
                .iter()
                .filter_map(|node| match node {
                    Node::Text(text) if text.trim().is_empty() => None,
                    Node::Text(_) => None, // Core ignores direct text outside token elements.
                    Node::Element(child)
                        if child.style.display == Some(crate::style::computed::Display::None) =>
                    {
                        None
                    }
                    Node::Element(child) if child.tag == "mprescripts" => None,
                    Node::Element(child) => {
                        let core = operators::core(child, &style, opts);
                        factories.push(core.filter(|op| op.flags & 1 != 0).map(|op| {
                            let child = child.clone();
                            let style = style.clone();
                            let opts = opts.clone();
                            let factory: stretch::Factory = Box::new(move |target| {
                                build(&child, &style, &opts, Some(target)).unwrap_or_else(|| {
                                    crate::render::element::element(&child, &style, &opts)
                                })
                            });
                            (op.flags, factory)
                        }));
                        let forwarded = if core.is_some() { target } else { None };
                        Some(build(child, &style, opts, forwarded).unwrap_or_else(|| {
                            crate::render::element::element(child, &style, opts)
                        }))
                    }
                })
                .collect();
        (children, parameters::kind(e, &style, opts))
    };
    let content = Content {
        children,
        factories,
        operator,
        kind,
        rtl: style.rtl == Some(true),
        width: px_value(style.width),
        height: px_value(style.height),
        offsets: vec![],
        roots: vec![],
        rule: None,
        color: style.color.map_or(opts.text.color, |c| c.to_hsla()),
        font: crate::text::inline::strut_font(&style, &opts.text),
        font_size: px_value(style.font_size).unwrap_or(opts.base_size()),
        glyph: None,
        glyph_offset: (0.0, 0.0),
    };
    let mut outer = crate::render::box_div::styled_div_with(e, &style);
    // The measured content owns its baseline, including an explicit zero.
    outer.style().align_items = Some(gpui::AlignItems::FlexStart);
    if e.tag == "math" && style.display == Some(crate::style::computed::Display::Block) {
        outer.style().flex_direction = Some(gpui::FlexDirection::Row);
        outer.style().justify_content = Some(gpui::JustifyContent::Center);
    }
    Some(outer.child(content).into_any_element())
}
