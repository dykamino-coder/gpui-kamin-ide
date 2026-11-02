//! Сертификация зависимостей по CSS и фактическому членству flex-линий.

use super::{Kind, Lines, Node, Serial};
use crate::dom::{Element, Node as DomNode};
use crate::layout::fragment::{
    flow::blocks::{BlockBox, Laid},
    types::{StackAxis, StackChild},
};
use crate::style::computed::{Display, FlexDir, Justify, Position};
use crate::style::values::value::Len;
use crate::tolerance::COORD_EPS;
use gpui::{Window, point, px, size};

impl Serial {
    pub(in super::super) fn measure(
        c: &StackChild,
        axis: StackAxis,
        window: &mut Window,
    ) -> Option<Self> {
        let r = c.regrow.as_ref()?;
        let b = r.watched.blocks()?;
        if axis.is_vertical()
            || !r.laid_h
            || c.line_trim.is_some()
            || c.over > c.h + COORD_EPS
            || b.flex.is_none()
        {
            return None;
        }
        let flow = r
            .watched
            .flow
            .at(point(px(0.), px(0.)), size(px(0.), px(c.h)), axis, window);
        if !flow.inline.is_empty()
            || !flow.solid.is_empty()
            || !flow.cuts.is_empty()
            || !flow.floats.is_empty()
            || !flow.line_boxes.is_empty()
        {
            return None;
        }
        let mut model = Self {
            nodes: Vec::new(),
            leaves: Vec::new(),
        };
        model.collect(&r.source, b, &r.laid)?;
        let root = model.nodes.last()?;
        ((root.bounds.0).abs() <= COORD_EPS && (root.bounds.1 - c.h).abs() <= COORD_EPS)
            .then_some(model)
    }

    fn collect(&mut self, e: &Element, b: &BlockBox, laid: &Laid) -> Option<usize> {
        let s = e.style.used();
        // Размер главной оси свободный; проценты, ограничения и исключения
        // могут замкнуть зависимость высоты на ширину или на размер предка.
        if b.clips_block
            || b.head != 0.
            || b.tail != 0.
            || !b.kids.is_empty()
            || b.table.is_some()
            || b.grid.is_some()
            || s.height.is_some()
            || s.min_height.is_some()
            || s.max_height.is_some()
            || s.aspect_ratio.is_some()
            || s.contain_size == Some(true)
            || s.contain_inline_size == Some(true)
            || s.writing_mode.is_some_and(|w| w.is_vertical())
            || s.position.is_some_and(|p| p != Position::Static)
            || s.flex_grow.is_some_and(|v| v != 0.)
            || s.flex_basis.is_some()
            || s.basis_content == Some(true)
            || s.float.is_some_and(|v| v != 0)
            || s.clear.is_some_and(|v| v != 0)
            || s.margin_trim != 0
            || ![s.margin.top, s.margin.bottom]
                .iter()
                .all(|v| matches!(v, None | Some(Len::Px(0.))))
            || crate::layout::box_kind::clamp_host(e, &s)
        {
            return None;
        }
        let bounds = laid.at(b.id)?;
        if !bounds.0.is_finite() || !bounds.1.is_finite() || bounds.1 < bounds.0 {
            return None;
        }
        let kind = if let Some(f) = &b.flex {
            if s.flex_dir != Some(FlexDir::Col)
                || s.flex_wrap == Some(true)
                || s.flex_balance == Some(true)
                || s.flex_wrap_reverse == Some(true)
                || s.flex_line_count.is_some_and(|n| n > 1)
                || !matches!(s.justify_content, None | Some(Justify::Start))
                || !matches!(s.gap, None | Some((None | Some(Len::Px(_)), _)))
            {
                return None;
            }
            let frame = laid.flexes.get(&b.id)?;
            let [line] = frame.lines.as_slice() else {
                return None;
            };
            if !frame.column
                || line.items.is_empty()
                || line.order.iter().any(|(_, id)| id.is_none())
            {
                return None;
            }
            let elements: Vec<_> = e
                .children
                .iter()
                .filter_map(|n| match n {
                    DomNode::Element(e) => Some(e),
                    _ => None,
                })
                .collect();
            if e.children
                .iter()
                .any(|n| matches!(n, DomNode::Text(t) if !t.trim().is_empty()))
                || elements.len() != f.items.len()
                || f.items.len() != line.items.len()
            {
                return None;
            }
            let mut kids = Vec::new();
            let mut end = bounds.0;
            let mut ordered = line.items.clone();
            ordered.sort_by(|a, b| {
                laid.at(*a)
                    .map(|p| p.0)
                    .unwrap_or(f32::NAN)
                    .total_cmp(&laid.at(*b).map(|p| p.0).unwrap_or(f32::NAN))
            });
            for id in &ordered {
                let k = f.items.iter().find(|k| k.id == *id)?;
                let e = elements.iter().find(|e| e.node_id == *id)?;
                let inner = k.inner.as_ref()?;
                let (top, bottom) = laid.at(*id)?;
                if k.monolith || (top - end).abs() > COORD_EPS {
                    return None;
                }
                kids.push(self.collect(e, inner, laid)?);
                end = bottom + f.main_gap;
            }
            if (end - f.main_gap - bounds.1).abs() > COORD_EPS {
                return None;
            }
            Kind::Column(kids)
        } else {
            if !matches!(s.display, None | Some(Display::Block))
                || !e.children.iter().all(|n| matches!(n, DomNode::Text(_)))
            {
                return None;
            }
            let flow = laid.flows.get(&b.id)?.clone();
            if flow.inline.first()?.lines.first()?.top != bounds.0 {
                return None;
            }
            let leaf = Lines::from_flow(flow, laid.lines.get(&b.id)?.clone(), bounds.1)?;
            let index = self.leaves.len();
            self.leaves.push(leaf);
            Kind::Leaf(index)
        };
        let index = self.nodes.len();
        self.nodes.push(Node {
            id: b.id,
            bounds,
            offset: 0.,
            kind,
        });
        Some(index)
    }
}
