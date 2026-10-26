//! Пункт списка: коробка `display: list-item` рисует свой маркер независимо от тега
//! родителя. Здесь слияние стиля пункта с наследуемым, установка слоёв строчных
//! псевдоэлементов и UA-правило CSS Lists 3 §3.1.1 `::marker { font-variant-numeric:
//! tabular-nums }`; разрешение текста маркера — в `marker`, вывод — в `styled`.

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{BoxStyle, Computed, Display};
use gpui::{AnyElement, Styled};
pub(crate) mod marker;
mod outside;
mod outside_flow;
mod placement;
mod styled;
pub(crate) use styled::render_with_style;

fn shrink0(d: gpui::Div, li: &Element, parent: &Computed) -> gpui::Div {
    let flex_parent = matches!(
        parent.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
    );
    if li.style.used().flex_shrink.is_none() && !flex_parent {
        d.flex_shrink_0()
    } else {
        d
    }
}

/// CSS Lists 3 §3.1.1 UA sheet: `::marker { font-variant-numeric: tabular-nums }`.
/// It overrides the item's inherited numeric variant; only an author
/// `::marker` rule naming a numeric variant replaces it.
fn tabular_marker(style: &mut Computed, item: &Computed, layer: Option<&Computed>) {
    const NUMERIC: [&str; 8] = [
        "lnum", "onum", "pnum", "tnum", "frac", "afrc", "ordn", "zero",
    ];
    let numeric = |t: &str| NUMERIC.contains(&t);
    if layer.is_some_and(|m| m.font_features.iter().any(|(t, _)| numeric(t))) {
        return;
    }
    let mut features = if style.font_features.is_empty() {
        item.font_features.clone()
    } else {
        std::mem::take(&mut style.font_features)
    };
    features.retain(|(t, _)| !numeric(t));
    features.push(("tnum".into(), 1));
    style.font_features = features;
}

pub(super) async fn render(
    stk: &mut reblessive::Stk,
    li: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let mut merged = box_style(inherited, &li.style, vcx);
    if matches!(
        li.style.used().display,
        None | Some(Display::Block | Display::ListItem | Display::TableCell)
    ) || (li.style.used().display == Some(Display::InlineBlock)
        && li.style.used().inline_display != Some(true))
    {
        crate::layout::box_tree::pseudo_line_layers::install(li, &mut merged, vcx);
    }
    render_with_style(stk, li, inherited, &merged, opts).await
}
