//! Отрисованная легенда `<fieldset>` (HTML §15.3.13): легенда стоит на блочной начальной
//! рамке, а не в потоке содержимого.

use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::style::computed::{BoxStyle, Display, Justify, TextAlign};
use crate::style::values::value::Len;

/// Дети `fieldset` с отрисованной легендой — первой `legend` в потоке — в полосе
/// легенды впереди содержимого. Полоса — гибкий ряд высотой не меньше начальной
/// рамки: легенда ниже рамки центрируется на ней, выше — начинается с края рамки
/// (Blink `FieldsetLayoutAlgorithm::LayoutLegend`). Полоса поднята через рамку и
/// отбивку к краю border box, а содержимое начинается под ней с отбивкой:
/// `max(рамка, легенда) + отбивка` (там же, `intrinsic_block_size_`). Отбивка и
/// рамка остаются декорацией самой коробки и повторяются на её фрагментах
/// (`box-decoration-break: clone`), легенда — только в первом.
pub(crate) fn with_rendered_legend(
    e: &Element,
    merged: &BoxStyle,
    children: Vec<Node>,
) -> Vec<Node> {
    if e.tag != "fieldset"
        || merged.wm().vertical()
        || crate::layout::multicol::spanner::multicol_container(merged)
        || !matches!(
            merged.display,
            None | Some(Display::Block | Display::InlineBlock)
        )
    {
        return children;
    }
    let Some(at) = children.iter().position(|n| {
        matches!(n, Node::Element(c) if c.tag == "legend"
            && !out_of_flow(&c.style.used())
            && c.style.used().float.unwrap_or(0) == 0)
    }) else {
        return children;
    };
    let mut children = children;
    let legend = children.remove(at);
    children.insert(0, legend_band(legend, merged));
    children
}

/// Полоса отрисованной легенды `legend` у `fieldset` со стилем `merged` (см.
/// `with_rendered_legend`); без разрешённых в точки рамки и отбивки — сама легенда.
pub(crate) fn legend_band(legend: Node, merged: &BoxStyle) -> Node {
    let px = |l: Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    match (px(merged.borders().top), px(merged.padding.top)) {
        (Some(border), Some(pad)) => Node::Element(band(legend, border, pad)),
        _ => legend,
    }
}

fn band(legend: Node, border: f32, pad: f32) -> Element {
    let justify = match &legend {
        Node::Element(l) => match l.style.used().text_align {
            Some(TextAlign::Center) => Some(Justify::Center),
            Some(TextAlign::Right | TextAlign::End) => Some(Justify::End),
            _ => None,
        },
        _ => None,
    };
    let style = crate::style::computed::legend_band::legend_band(border, pad, justify);

    let node_id = match &legend {
        Node::Element(l) => l.node_id ^ LEGEND_BAND_ID,
        _ => 0,
    };
    let mut band = Element::anonymous("div", style, vec![legend], false, Default::default());
    band.node_id = node_id;
    band
}

/// Номер полосы легенды `id` — `id ^ LEGEND_BAND_ID` (своя коробка раскладки).
const LEGEND_BAND_ID: u64 = 0x4c45_4745_4e44_0001;

#[cfg(test)]
mod tests {
    use super::with_rendered_legend;
    use crate::dom::{Element, Node};
    use crate::style::computed::{BoxStyle, Computed, Display, Sides};
    use crate::style::values::value::Len;

    fn el(tag: &str) -> Node {
        Node::Element(Element::synthetic(
            tag,
            Computed::default(),
            Default::default(),
            vec![],
        ))
    }

    #[test]
    fn the_first_flow_legend_moves_into_a_band_over_the_start_border() {
        let fieldset =
            Element::synthetic("fieldset", Computed::default(), Default::default(), vec![]);
        let merged = BoxStyle::shared(std::rc::Rc::new(Computed {
            border_width: Sides {
                top: Some(Len::Px(5.0)),
                ..Default::default()
            },
            border_visible: [Some(true); 4],
            padding: Sides {
                top: Some(Len::Px(2.0)),
                ..Default::default()
            },
            ..Default::default()
        }));
        let kids = with_rendered_legend(
            &fieldset,
            &merged,
            vec![el("div"), el("legend"), el("legend")],
        );
        let Node::Element(band) = &kids[0] else {
            panic!()
        };
        assert_eq!(band.style.used().display, Some(Display::Flex));
        assert_eq!(band.style.used().min_height, Some(Len::Px(5.0)));
        assert_eq!(band.style.used().margin.top, Some(Len::Px(-7.0)));
        assert_eq!(band.style.used().margin.bottom, Some(Len::Px(2.0)));
        assert!(matches!(&band.children[0], Node::Element(l) if l.tag == "legend"));
        let tags: Vec<_> = kids[1..]
            .iter()
            .map(|n| match n {
                Node::Element(e) => e.tag.to_string(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(tags, ["div", "legend"]);
    }
}
