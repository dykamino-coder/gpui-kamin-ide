//! Quirks Mode §§3.6–3.7: minimum used block sizes of the document root and body.

use crate::dom::Element;
use crate::render::RenderOpts;
use crate::style::computed::{Computed, Display, Position};
use crate::style::values::value::Len;

pub(super) fn fill(e: &Element, st: &mut Computed, parent: &Computed, opts: &RenderOpts) {
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
        opts.cx.avail_w
    };
    let px = |value: Option<Len>, style: &Computed| match value {
        None | Some(Len::Auto) => 0.0,
        Some(v) => v.to_px(basis, style.font_cx(), vcx).unwrap_or(0.0),
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
            _ if !vertical && opts.cx.avail_h.is_some() => opts.cx.avail_h.unwrap_or(0.0),
            Some(v) if v != Len::Auto => {
                let size = v
                    .to_px(Some(viewport), parent.font_cx(), vcx)
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
        .and_then(|v| v.to_px(Some(available), st.font_cx(), vcx))
        .unwrap_or(0.0);
    let minimum = Len::Px(minimum.max(old));
    if vertical {
        st.min_width = Some(minimum);
    } else {
        st.min_height = Some(minimum);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_exclusion_survives_float_layout_clearing_the_property() {
        let nodes = crate::dom::parse(
            "<!DOCTYPE quirks-mode><style>body { float: left }</style><body></body>",
            "",
        );
        let crate::dom::Node::Element(html) = &nodes[0] else {
            panic!("html")
        };
        let body = html
            .children
            .iter()
            .find_map(|n| match n {
                crate::dom::Node::Element(e) if e.tag == "body" => Some(e),
                _ => None,
            })
            .expect("body");
        let mut body = body.clone();
        body.style.float = None;
        let mut st = Computed::default();
        fill(&body, &mut st, &Computed::default(), &opts());
        assert_eq!(st.min_height, None);
    }

    fn opts() -> RenderOpts {
        RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: (800.0, 600.0),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        }
    }

    #[test]
    fn fill_is_a_content_minimum_and_does_not_make_auto_height_definite() {
        let mut body = crate::layout::table::anon::anon_element("body", vec![]);
        body.marks.quirks_fill = true;
        let mut st = Computed::default();
        st.margin.top = Some(Len::Px(10.0));
        st.margin.bottom = Some(Len::Px(12.0));
        st.padding.top = Some(Len::Px(5.0));
        let parent = Computed {
            height: Some(Len::Px(200.0)),
            ..Default::default()
        };
        fill(&body, &mut st, &parent, &opts());
        assert_eq!(st.min_height, Some(Len::Px(173.0)));
        assert_eq!(st.height, None);
        assert_eq!(
            crate::layout::block::available_height::inner(&st, Some(200.0), Some(800.0), 600.0,),
            None
        );
    }

    #[test]
    fn inline_floating_and_positioned_bodies_keep_their_normal_size() {
        let mut body = crate::layout::table::anon::anon_element("body", vec![]);
        body.marks.quirks_fill = true;
        for mut st in [
            Computed {
                display: Some(Display::InlineBlock),
                ..Default::default()
            },
            Computed {
                display: Some(Display::InlineTable),
                ..Default::default()
            },
            Computed {
                float: Some(1),
                ..Default::default()
            },
            Computed {
                position: Some(Position::Absolute),
                ..Default::default()
            },
            Computed {
                position: Some(Position::Fixed),
                ..Default::default()
            },
        ] {
            body.style = st.clone().into();
            fill(&body, &mut st, &Computed::default(), &opts());
            assert_eq!(st.min_height, None);
        }
    }

    #[test]
    fn vertical_fill_uses_width_and_preserves_border_box_minimum() {
        let mut body = crate::layout::table::anon::anon_element("body", vec![]);
        body.marks.quirks_fill = true;
        let doc = crate::document::state::DocState::new(Default::default());
        let mut st = Computed::default();
        st.apply_one("writing-mode", "vertical-lr", doc.values());
        st.apply_one("box-sizing", "border-box", doc.values());
        st.apply_one("padding", "5px", doc.values());
        st.apply_one("margin", "10px", doc.values());
        let parent = Computed {
            width: Some(Len::Px(200.0)),
            ..Default::default()
        };
        fill(&body, &mut st, &parent, &opts());
        assert_eq!(st.min_width, Some(Len::Px(180.0)));
        assert_eq!(st.width, None);
    }

    #[test]
    fn calc_size_auto_does_not_trigger_either_fill_axis() {
        let mut body = crate::layout::table::anon::anon_element("body", vec![]);
        body.marks.quirks_fill = true;
        let doc = crate::document::state::DocState::new(Default::default());
        for (mode, property) in [("horizontal-tb", "height"), ("vertical-lr", "width")] {
            let mut st = Computed::default();
            st.apply_one("writing-mode", mode, doc.values());
            st.apply_one(property, "calc-size(auto, size)", doc.values());
            fill(&body, &mut st, &Computed::default(), &opts());
            assert_eq!(st.min_width, None);
            assert_eq!(st.min_height, None);
        }
    }
}
