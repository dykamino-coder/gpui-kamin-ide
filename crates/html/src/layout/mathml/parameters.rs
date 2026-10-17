//! Resolved MathML length attributes and formatting-context parameters.
use super::{geometry::Metrics, padded};
use crate::style::cascade::inherit::inherit;
use crate::{
    dom::Element,
    render::RenderOpts,
    style::{computed::Computed, values::value::Len},
};

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Row,
    Operator(f32, f32),
    Space(Metrics),
    Padded(padded::Parameters),
    Fraction(super::fraction::Parameters),
    Scripts(super::scripts::Parameters, ScriptKind),
    Limits(super::limits::Parameters, ScriptKind),
    Radical(super::radical::Parameters),
}

#[derive(Clone, Copy)]
pub(super) enum ScriptKind {
    Sub,
    Sup,
    Both,
    Multi(Option<usize>),
}

pub(super) fn kind(e: &Element, style: &Computed, opts: &RenderOpts) -> Kind {
    let attr = |name| e.attr(name).and_then(|v| length_px(v, style, opts));
    if e.tag == "mpadded" {
        Kind::Padded(padded::Parameters {
            width: px_value(style.width),
            ascent: attr("height"),
            descent: attr("depth"),
            lspace: attr("lspace").unwrap_or(0.0),
            voffset: attr("voffset").unwrap_or(0.0),
            css_width: px_value(style.width),
            css_height: px_value(style.height),
            rtl: style.rtl == Some(true),
        })
    } else {
        let font = super::font_parameters::FontParameters::new(e, style, opts);
        let count = e
            .children
            .iter()
            .filter(|n| matches!(n, crate::dom::Node::Element(_)))
            .count();
        let movable = style.math_compact != Some(false)
            && e.children
                .iter()
                .find_map(|n| {
                    if let crate::dom::Node::Element(e) = n {
                        Some(e)
                    } else {
                        None
                    }
                })
                .and_then(|base| super::operators::core(base, style, opts))
                .is_some_and(|op| op.flags & 8 != 0);
        match (e.tag.as_str(), count) {
            ("msqrt", _) | ("mroot", 2) => Kind::Radical(font.radical),
            ("mfrac", 2) => Kind::Fraction(font.fraction),
            ("msub", 2) => Kind::Scripts(font.scripts, ScriptKind::Sub),
            ("msup", 2) => Kind::Scripts(font.scripts, ScriptKind::Sup),
            ("msubsup", 3) => Kind::Scripts(font.scripts, ScriptKind::Both),
            ("munder", 2) if movable => Kind::Scripts(font.scripts, ScriptKind::Sub),
            ("mover", 2) if movable => Kind::Scripts(font.scripts, ScriptKind::Sup),
            ("munderover", 3) if movable => Kind::Scripts(font.scripts, ScriptKind::Both),
            ("munder", 2) => Kind::Limits(font.limits, ScriptKind::Sub),
            ("mover", 2) => Kind::Limits(font.limits, ScriptKind::Sup),
            ("munderover", 3) => Kind::Limits(font.limits, ScriptKind::Both),
            ("mmultiscripts", n) if n > 0 => {
                let marker = e
                    .children
                    .iter()
                    .filter_map(|n| {
                        if let crate::dom::Node::Element(e) = n {
                            Some(e)
                        } else {
                            None
                        }
                    })
                    .position(|e| e.tag == "mprescripts");
                if marker.is_some_and(|m| m == 0 || (m - 1) % 2 != 0 || (n - m - 1) % 2 != 0)
                    || (marker.is_none() && (n - 1) % 2 != 0)
                {
                    Kind::Row
                } else {
                    Kind::Scripts(font.scripts, ScriptKind::Multi(marker))
                }
            }
            _ => Kind::Row,
        }
    }
}

pub(super) fn px_value(value: Option<Len>) -> Option<f32> {
    if let Some(Len::Px(v)) = value {
        Some(v)
    } else {
        None
    }
}

pub(super) fn length_px(raw: &str, style: &Computed, opts: &RenderOpts) -> Option<f32> {
    let value = crate::dom::mathml::length(raw)?;
    let own = Computed {
        width: Some(value),
        ..Computed::default()
    };
    let mut resolved = inherit(style, &own);
    resolved.resolve_viewport(opts.viewport);
    px_value(resolved.width)
}
