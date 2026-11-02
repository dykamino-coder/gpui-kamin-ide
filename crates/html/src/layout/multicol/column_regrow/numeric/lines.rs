//! Чистый IFC постоянной ширины: расход перед строкой не меняет её содержимое.

use crate::dom::Node;
use crate::layout::fragment::flow::{FlowGeometry, LinePush, blocks::Pushed};
use crate::layout::fragment::push::Growth;
use crate::layout::fragment::types::{Kid, StackAxis, StackChild};
use crate::tolerance::COORD_EPS;
use gpui::{Window, point, px, size};

pub(super) struct Lines {
    flow: FlowGeometry,
    pub(super) pushes: Vec<LinePush>,
    pub(super) extra: Vec<f32>,
}

impl Lines {
    pub(super) fn measure(
        child: &StackChild,
        axis: StackAxis,
        window: &mut Window,
    ) -> Option<Self> {
        let r = child.regrow.as_ref()?;
        let b = r.watched.blocks()?;
        let style = r.source.style.used();
        // Ширину строк могут менять exclusions и атомы; ограниченный блочный
        // размер не обязан расти вместе с IFC. Такие зависимости здесь не решаются.
        if axis.is_vertical()
            || !r.laid_h
            || b.clips_block
            || !b.kids.is_empty()
            || b.table.is_some()
            || b.grid.is_some()
            || b.flex.is_some()
            || b.head != 0.
            || b.tail != 0.
            || child.line_trim.is_some()
            || child.over > child.h + COORD_EPS
            || style.height.is_some()
            || style.min_height.is_some()
            || style.max_height.is_some()
            || style.aspect_ratio.is_some()
            || style.contain_size == Some(true)
            || crate::layout::box_kind::clamp_host(&r.source, &style)
            || !r.source.children.iter().all(|n| matches!(n, Node::Text(_)))
        {
            return None;
        }
        let flow = r.watched.flow.at(
            point(px(0.), px(0.)),
            size(px(0.), px(child.h)),
            axis,
            window,
        );
        Self::from_flow(flow, r.laid.lines.get(&b.id)?.clone(), child.h)
    }

    pub(super) fn from_flow(flow: FlowGeometry, pushes: Vec<LinePush>, end: f32) -> Option<Self> {
        let [paragraph] = flow.inline.as_slice() else {
            return None;
        };
        if !flow.floats.is_empty()
            || !flow.line_boxes.is_empty()
            || paragraph.lines.len() != pushes.len()
            || pushes.is_empty()
            || paragraph.lines.iter().any(|l| l.consumed_before != 0.)
            || paragraph
                .lines
                .last()
                .is_none_or(|l| (l.bottom - end).abs() > COORD_EPS)
            || paragraph
                .lines
                .iter()
                .zip(&pushes)
                .any(|(l, p)| (l.top - p.at).abs() > COORD_EPS)
        {
            return None;
        }
        Some(Self {
            extra: vec![0.; pushes.len()],
            flow,
            pushes,
        })
    }

    pub(super) fn reset(&mut self) {
        self.extra.fill(0.);
    }

    pub(super) fn accepts(&self, grows: &[Growth]) -> bool {
        grows.iter().all(|g| {
            g.amount.is_finite()
                && g.amount > 0.
                && matches!(&g.target, Pushed::Line(p) if self.pushes.contains(p))
        })
    }

    pub(super) fn grow(&mut self, child: &mut StackChild, base: &Kid, grows: Vec<Growth>) {
        self.add(&grows);
        let flow = self.geometry(0.);
        let total = self.total();
        child.h = base.h + total;
        child.over = base.over + total;
        let r = child.regrow.as_mut().expect("measured line model");
        let b = r.watched.blocks().expect("measured line scope");
        if let Some(&(top, _)) = r.laid.boxes.get(&b.id) {
            r.laid.boxes.insert(b.id, (top, top + child.h));
        }
        r.laid.lines.insert(b.id, flow.line_pushes.clone());
        let breaks = b.breaks(child.h, &r.laid).finish(flow, child.h);
        super::model::record(child, breaks);
    }

    pub(super) fn add(&mut self, grows: &[Growth]) {
        // Как Regrow::push_many: одинаковые адреса получают максимум, не сумму.
        let mut amounts = vec![0f32; self.extra.len()];
        for growth in grows {
            if let Pushed::Line(line) = &growth.target
                && let Some(index) = self.pushes.iter().position(|p| p == line)
            {
                amounts[index] = amounts[index].max(growth.amount);
            }
        }
        for (extra, amount) in self.extra.iter_mut().zip(amounts) {
            *extra += amount;
        }
    }

    pub(super) fn total(&self) -> f32 {
        self.extra.iter().sum()
    }

    /// Смещение владельца не меняет локальный consumed prefix самого IFC.
    pub(super) fn geometry(&self, offset: f32) -> FlowGeometry {
        let mut flow = self.flow.clone();
        // Координаты окончания предыдущей строки исключают новое место;
        // начало следующей и точки продолжения включают его (§5.3).
        let shift = |at: f32, inclusive: bool| -> f32 {
            offset
                + at
                + self
                    .pushes
                    .iter()
                    .zip(&self.extra)
                    .filter(|(p, _)| {
                        if inclusive {
                            p.at <= at + COORD_EPS
                        } else {
                            p.at < at - COORD_EPS
                        }
                    })
                    .map(|(_, extra)| *extra)
                    .sum::<f32>()
        };
        for cut in &mut flow.cuts {
            *cut = (shift(cut.0, true), shift(cut.1, true));
        }
        for range in &mut flow.solid {
            *range = (shift(range.0, true), shift(range.1, false));
        }
        let mut prefix = 0.;
        for (line, extra) in flow.inline[0].lines.iter_mut().zip(&self.extra) {
            line.prefix += prefix;
            line.top = shift(line.top, true);
            line.bottom = shift(line.bottom, false);
            line.consumed_before = *extra;
            prefix += extra;
        }
        let mut pushes = self.pushes.clone();
        for push in &mut pushes {
            push.at = shift(push.at, true);
        }
        if self.extra[0] > 0. {
            flow.cuts.push((pushes[0].at, pushes[0].at));
        }
        flow.line_pushes = pushes;
        flow.normalize();
        flow
    }

    pub(super) fn commit(&self) -> bool {
        let mut changed = false;
        for (push, &extra) in self.pushes.iter().zip(&self.extra) {
            changed |= push.grow(extra);
        }
        changed
    }
}
