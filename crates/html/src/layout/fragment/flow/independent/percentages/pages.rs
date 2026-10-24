//! Исходные CB до снятия обёрток: зависимости размера без копии дочернего дерева.

use super::{Computed, freeze};
use crate::dom::Node;
use crate::layout::block::{available_height, available_width};
use crate::layout::fragment::flow::blocks::leaf::{Edges, Leaf};
use crate::layout::fragment::token::Space;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct PagePercentages(Rc<HashMap<u64, Rc<Context>>>);

pub(crate) struct Context {
    parents: Vec<Rc<Computed>>,
    style: Rc<Computed>,
}

impl PagePercentages {
    pub(crate) fn collect(nodes: &[Node], parent: &BoxStyle, opts: &RenderOpts) -> Self {
        fn walk(
            nodes: &[Node],
            parent: &BoxStyle,
            opts: &RenderOpts,
            parents: &mut Vec<Rc<Computed>>,
            map: &mut HashMap<u64, Rc<Context>>,
        ) {
            for node in nodes {
                if let Node::Element(e) = node {
                    let boxed = crate::style::cascade::materialize::box_style(
                        parent,
                        &e.style,
                        opts.cx.vcx(),
                    );
                    map.insert(
                        e.node_id,
                        Rc::new(Context {
                            parents: parents.clone(),
                            style: boxed.style_rc().clone(),
                        }),
                    );
                    parents.push(boxed.style_rc().clone());
                    walk(&e.children, &boxed, opts, parents, map);
                    parents.pop();
                }
            }
        }
        let mut map = HashMap::new();
        walk(
            nodes,
            parent,
            opts,
            &mut vec![parent.style_rc().clone()],
            &mut map,
        );
        Self(Rc::new(map))
    }

    /// Обновляет размеры существующей continuation, сохраняя её child identities.
    pub(crate) fn refresh(&self, e: &mut crate::dom::Element, space: Space, opts: &RenderOpts) {
        let context = self.context(e.node_id);
        let style = context.style(space, opts);
        let used = e.style.used_mut();
        (used.height, used.min_height, used.max_height) =
            (style.height, style.min_height, style.max_height);
        if matches!(
            style.display,
            None | Some(
                crate::style::computed::Display::Block
                    | crate::style::computed::Display::ListItem
                    | crate::style::computed::Display::Contents
            )
        ) && !style.wm().vertical()
            && style.column_count.is_none()
            && style.column_width.is_none()
        {
            for node in &mut e.children {
                if let Node::Element(child) = node
                    && !child.inline
                    && !crate::layout::box_kind::out_of_flow(&child.style.used())
                {
                    self.refresh(child, space, opts);
                }
            }
        }
    }

    pub(crate) fn context(&self, id: u64) -> Rc<Context> {
        self.0
            .get(&id)
            .expect("leaf retains its original containing-block chain")
            .clone()
    }
}

impl Context {
    /// css-break-3 §5.1: новый лист меняет размеры, но не исходную цепочку CB.
    fn resolved(&self, space: Space, opts: &RenderOpts) -> (Computed, Option<f32>) {
        let viewport = (space.inline_size.unwrap(), space.block_size.unwrap());
        let (mut width, mut height) = (opts.cx.constraints.avail_w, opts.cx.constraints.avail_h);
        for parent in &self.parents {
            let next_height = if parent.icb {
                Some(viewport.1)
            } else {
                available_height::inner(parent, height, width, viewport.1)
            };
            width = if parent.icb {
                Some(viewport.0)
            } else {
                available_width::inner(parent, width)
            };
            height = next_height;
        }
        let mut style = (*self.style).clone();
        let base = if style.root_box {
            Some(viewport.1)
        } else {
            height
        };
        freeze(&mut style, base);
        (style, width)
    }

    fn style(&self, space: Space, opts: &RenderOpts) -> Computed {
        self.resolved(space, opts).0
    }

    pub(crate) fn leaf(&self, space: Space, opts: &RenderOpts) -> (Leaf, Computed) {
        let (style, width) = self.resolved(space, opts);
        let side = |value| match value {
            Some(Len::Px(value)) => value,
            Some(Len::Pct(value)) => value * width.expect("block containing width"),
            _ => 0.0,
        };
        let border = style.borders();
        let edges = Edges {
            start: side(style.padding.top) + side(border.top),
            end: side(style.padding.bottom) + side(border.bottom),
            clone: style.bdb_clone,
        };
        let subtract = if style.border_box == Some(true) {
            edges.sum()
        } else {
            0.0
        };
        let content = |value| match value {
            Some(Len::Px(value)) => Some((value - subtract).max(0.0)),
            _ => None,
        };
        let content_block = content(style.height)
            .unwrap_or(0.0)
            .min(content(style.max_height).unwrap_or(f32::INFINITY))
            .max(content(style.min_height).unwrap_or(0.0));
        (
            Leaf {
                content_block,
                edges,
            },
            style,
        )
    }
}
