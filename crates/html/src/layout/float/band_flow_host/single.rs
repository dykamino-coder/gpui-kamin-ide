//! Единственный float без обтекания: тот же shrink-to-fit в общем дереве layout.

use super::*;
use gpui::px;

/// CSS 2.1 §10.3.5: поперечный auto-размер flex-элемента с start-выравниванием
/// равен shrink-to-fit. Без соседей не нужны полосы и независимые деревья проб.
pub(super) async fn build(
    stk: &mut reblessive::Stk,
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    let host = e.marks.shape_flow?;
    if host.count != 1
        || host.wm.vertical()
        || opts.frag.watch.is_some()
        || opts.cx.clamp_measure().is_some()
    {
        return None;
    }
    let mut children = e
        .children
        .iter()
        .filter(|n| !matches!(n, Node::Text(t) if t.trim().is_empty()));
    let Node::Element(node) = children.next()? else {
        return None;
    };
    if children.next().is_some() {
        return None;
    }
    let c = node.style.used();
    let side = c.float.filter(|f| *f != 0)?;
    let zero = |v| matches!(v, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    // Доли и особые форматирующие контексты пока требуют ширины из плана полос.
    let absolute = |v| matches!(v, None | Some(Len::Auto) | Some(Len::Px(_)));
    if own_layout_mode(&c)
        || replaced_tag(node)
        || c.display == Some(Display::ListItem)
        || c.wm().vertical()
        || c.clamp_lines().is_some()
        || c.clamp_by_height()
        || ![c.margin.top, c.margin.right, c.margin.bottom, c.margin.left]
            .into_iter()
            .all(zero)
        || ![
            c.width,
            c.height,
            c.min_width,
            c.max_width,
            c.min_height,
            c.max_height,
            c.padding.top,
            c.padding.right,
            c.padding.bottom,
            c.padding.left,
        ]
        .into_iter()
        .all(absolute)
    {
        return None;
    }
    let mut copy = node.clone();
    let c = copy.style.used_mut();
    c.float = None;
    c.floated_box = true;
    c.flex_shrink = Some(0.0);
    c.band_kid = true;
    c.clear = None;
    c.margin = Default::default();
    c.flow_root = Some(true);
    let mut parent = inherited.clone();
    if let Some(h) = host.cbh {
        parent.height = Some(Len::Px(h));
    }
    if let Some(w) = host.cbw {
        parent.width = Some(Len::Px(w));
    }
    let child_opts = opts.inner(|cx| cx.clamp.build = cx.clamp.build.bfc());
    let child =
        super::float_box::float_box_stack(stk, &copy, node, 0.0, &parent, &child_opts).await;
    let child = opts
        .cx
        .contracts
        .build
        .effects(child, Effect::OutlineScope(&opts.cx.deferred.paint));
    let mut frame = frame(side, child);
    if e.marks.inflow_height {
        frame = frame.h(px(0.0));
    }
    if host.icb {
        frame = frame.w_full();
    }
    Some(opts.cx.contracts.build.effects(
        frame.into_any_element(),
        Effect::OutlineFlow(&opts.cx.deferred.paint),
    ))
}

fn frame(side: i8, child: AnyElement) -> gpui::Div {
    let mut frame = gpui::div().flex().flex_col().child(child);
    // Физические стороны Computed: -1 — left, +1 — right.
    frame = if side < 0 {
        frame.items_start()
    } else {
        frame.items_end()
    };
    frame.style().paint_atomic = Some(true);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{IntoHooked, Wrap};
    use std::{cell::Cell, rc::Rc};

    struct Capture {
        child: AnyElement,
        bounds: Rc<Cell<Option<gpui::Bounds<gpui::Pixels>>>>,
    }

    impl Wrap for Capture {
        fn child(&mut self) -> &mut AnyElement {
            &mut self.child
        }

        fn prepaint(
            &mut self,
            bounds: gpui::Bounds<gpui::Pixels>,
            window: &mut gpui::Window,
            cx: &mut gpui::App,
        ) {
            self.bounds.set(Some(bounds));
            self.child.prepaint(window, cx);
        }
    }

    #[gpui::test]
    fn isolated_float_uses_its_physical_side_without_shrinking(cx: &mut gpui::TestAppContext) {
        for (side, expected_x) in [(-1, 0.), (1, 80.)] {
            let bounds = Rc::new(Cell::new(None));
            let child = Capture {
                child: gpui::div()
                    .w(px(20.))
                    .h(px(30.))
                    .flex_shrink_0()
                    .into_any_element(),
                bounds: bounds.clone(),
            };
            cx.add_empty_window().draw(
                gpui::point(px(0.), px(0.)),
                gpui::size(px(100.), px(100.)),
                |_, _| frame(side, child.hooked()).w(px(100.)).h(px(0.)),
            );
            assert_eq!(
                bounds.get().expect("float reached prepaint"),
                gpui::Bounds::new(
                    gpui::point(px(expected_x), px(0.)),
                    gpui::size(px(20.), px(30.)),
                ),
                "float side {side}",
            );
        }
    }

    #[gpui::test]
    fn isolated_floating_table_keeps_its_border_spacing(cx: &mut gpui::TestAppContext) {
        let document = crate::Document::builder(
            "<table style='float:left;border-spacing:7px 9px'><tr>\
             <td style='width:20px;height:30px;padding:0'></td></tr></table>",
            "",
        )
        .build()
        .unwrap();
        let mut pending: Vec<_> = document.nodes().iter().collect();
        let source = loop {
            let Node::Element(node) = pending.pop().expect("table in document") else {
                continue;
            };
            if node.tag == "table" {
                break node;
            }
            pending.extend(node.children.iter());
        };
        let mut copy = source.clone();
        copy.style.used_mut().float = None;
        copy.style.used_mut().floated_box = true;
        let opts = RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: (100., 100.),
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        let bounds = Rc::new(Cell::new(None));
        let child = Capture {
            child: super::super::float_box::float_box(
                &copy,
                source,
                100.,
                &BoxStyle::default(),
                &opts,
            ),
            bounds: bounds.clone(),
        };
        cx.add_empty_window().draw(
            gpui::point(px(0.), px(0.)),
            gpui::size(px(100.), px(100.)),
            |_, _| frame(-1, child.hooked()).w(px(100.)).h(px(0.)),
        );
        assert_eq!(
            bounds.get().expect("table reached prepaint").size.width,
            px(34.),
        );
    }
}
