//! Проценты normal flow разрешаются от исходного CB до MaxContent-меры копий.

use crate::dom::{Element, Node};
use crate::layout::block::{available_height, available_width};
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Computed, Display};
use crate::style::values::value::Len;
mod pages;
pub(crate) use pages::PagePercentages;

pub(crate) fn page_percentages(
    nodes: &[Node],
    parent: &BoxStyle,
    opts: &RenderOpts,
) -> (Vec<Node>, PagePercentages) {
    crate::style::cascade::materialize::materialize(nodes, parent.style_rc(), opts.cx.vcx());
    let contexts = PagePercentages::collect(nodes, parent, opts);
    let nodes = nodes
        .iter()
        .map(|n| match n {
            Node::Element(e) if !crate::layout::box_kind::out_of_flow(&e.style.used()) => {
                Node::Element(block_percentages(e, parent, opts))
            }
            other => other.clone(),
        })
        .collect();
    (nodes, contexts)
}

fn block_percentages(e: &Element, parent: &BoxStyle, opts: &RenderOpts) -> Element {
    let mut parent = parent.clone();
    parent.resolve_viewport(opts.viewport, opts.cx.vcx());
    let constraints = &opts.cx.constraints;
    let height = if parent.icb {
        Some(opts.viewport.1)
    } else {
        available_height::inner(
            &parent,
            constraints.avail_h,
            constraints.avail_w,
            opts.viewport.1,
        )
    };
    let width = if parent.icb {
        Some(opts.viewport.0)
    } else {
        available_width::inner(&parent, constraints.avail_w)
    };
    resolve(e, &parent, height, width, opts)
}

/// Колонка передаёт размер исходного CB отдельно от доступного остатка.
pub(crate) fn column_percentages(
    nodes: &[Node],
    parent: &BoxStyle,
    width: Option<f32>,
    opts: &RenderOpts,
) -> Vec<Node> {
    let mut ordinary = parent.clone();
    ordinary.resolve_viewport(opts.viewport, opts.cx.vcx());
    ordinary.column_count = None;
    ordinary.column_width = None;
    let height = available_height::inner(
        &ordinary,
        opts.cx.constraints.avail_h,
        opts.cx.constraints.avail_w,
        opts.viewport.1,
    );
    nodes
        .iter()
        .map(|n| match n {
            Node::Element(e)
                if !e.inline && !crate::layout::box_kind::out_of_flow(&e.style.used()) =>
            {
                // Процентная высота использует content box многоколоночника,
                // ширина — колонку (Blink ColumnPercentageResolutionSize).
                Node::Element(resolve(e, parent, height, width, opts))
            }
            other => other.clone(),
        })
        .collect()
}

fn resolve(
    e: &Element,
    parent: &BoxStyle,
    height: Option<f32>,
    width: Option<f32>,
    opts: &RenderOpts,
) -> Element {
    let mut element = e.clone();
    let mut used = crate::style::cascade::materialize::box_style(parent, &e.style, opts.cx.vcx());
    used.resolve_viewport(opts.viewport, opts.cx.vcx());
    // Физическая height вертикальной коробки — её строчная ось;
    // процент разрешает вертикальная раскладка, а не блочная цепочка CSS2.
    if used.wm().vertical() {
        return element;
    }
    let base = if used.root_box {
        Some(opts.viewport.1)
    } else {
        height
    };
    freeze(element.style.used_mut(), base);
    freeze(&mut used, base);
    // Flex/grid/table вычисляют определённость размеров своих items собственным
    // алгоритмом; CSS2 §10.5 относится здесь к детям блочного содержащего блока.
    if matches!(
        used.display,
        None | Some(Display::Block | Display::ListItem | Display::Contents)
    ) && !used.wm().vertical()
        && used.column_count.is_none()
        && used.column_width.is_none()
    {
        let next_height = available_height::inner(&used, height, width, opts.viewport.1);
        let next_width = available_width::inner(&used, width);
        element.children = e
            .children
            .iter()
            .map(|n| match n {
                Node::Element(k)
                    if !k.inline && !crate::layout::box_kind::out_of_flow(&k.style.used()) =>
                {
                    Node::Element(resolve(k, &used, next_height, next_width, opts))
                }
                other => other.clone(),
            })
            .collect();
    }
    element
}

fn freeze(style: &mut Computed, base: Option<f32>) {
    if let Some(Len::Pct(k)) = style.height {
        style.height = base.map(|h| Len::Px(k * h));
    }
    // CSS2 §10.7: при неопределённом CB процентный min-height равен 0,
    // max-height — none. Min-height родителя не делает его height определённым.
    if let Some(Len::Pct(k)) = style.min_height {
        style.min_height = Some(Len::Px(base.map_or(0.0, |h| k * h)));
    }
    if let Some(Len::Pct(k)) = style.max_height {
        style.max_height = base.map(|h| Len::Px(k * h));
    }
}

#[cfg(test)]
mod tests;
