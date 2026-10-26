//! Границы фрагментов класса B по фактическим строковым боксам абзаца.

use super::Paragraph;
use crate::layout::fragment::flow::{FlowGeometry, FlowMeasure, FlowNode};
use crate::layout::fragment::types::StackAxis;
use std::rc::Rc;

/// The paragraph's line boxes in a measured fragmentation flow, with the
/// `orphans`/`widows` of its block container (css-break-3 §4.4).
#[derive(Clone, Debug)]
pub(crate) struct LineFlow {
    pub(crate) node: FlowNode,
    keep: (usize, usize),
}

impl Paragraph {
    /// Consumed fragmentainer space belongs above the continued line box.
    pub(super) fn fragment_padding(&self, mut pads: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
        if self.fragment.is_some() {
            for (index, (above, _)) in pads.iter_mut().enumerate() {
                *above += self.fragment_leading(index);
            }
        }
        pads
    }

    pub(super) fn fragment_line_index(&self) -> usize {
        self.fragment
            .as_ref()
            .and_then(|f| f.node.resume.state())
            .map_or(0, |state| state.line_idx)
    }

    /// Префикс — только перевод координат; он не создаёт line boxes (§4.4).
    pub(super) fn fragment_leading(&self, index: usize) -> f32 {
        self.fragment.as_ref().map_or(0.0, |f| {
            let prefix = if index == 0 {
                f.node.resume.consumed()
            } else {
                0.0
            };
            prefix
                + f.node
                    .growth
                    .borrow()
                    .get(self.fragment_line_index() + index)
                    .copied()
                    .unwrap_or(0.0)
        })
    }

    pub(crate) fn fragment_flow(
        mut self,
        flow: Option<Rc<FlowMeasure>>,
        axis: StackAxis,
        keep: (usize, usize),
    ) -> Self {
        self.fragment = flow.filter(|f| f.lines).map(|f| LineFlow {
            node: FlowNode::new(f, axis),
            keep,
        });
        self
    }

    pub(super) fn publish_fragment_lines(&self, pads: &[(f32, f32)], height: f32) {
        if let Some(LineFlow { node, keep }) = &self.fragment {
            // CSS Break 3 section 4.1: break between line boxes. The same
            // padding accounts for atoms, emphasis and ruby in the box size.
            let mut geometry = FlowGeometry::lines(pads.iter().map(|(a, b)| height + a + b));
            // CSS 2.1 section 9.5: avoidance is free space before a real
            // line, so a fragment cut may lie at the end of that space.
            let growth = node.growth.borrow();
            let first = self.fragment_line_index();
            let mut starts = node.starts.borrow_mut();
            starts.clear();
            for (index, (solid, line)) in geometry.solid.iter_mut().zip(&self.lines).enumerate() {
                let consumed = growth.get(first + index).copied().unwrap_or(0.0);
                // Остаток принадлежит предыдущему фрагменту, а не line box.
                // Его прежний конец больше не является точкой разрыва.
                if index > 0 && consumed > 0.0 {
                    geometry.cuts[index - 1].0 += consumed;
                    geometry.cuts[index - 1].1 += consumed;
                }
                solid.0 += self.fragment_leading(index);
                starts.push((first + index, solid.0));
                solid.0 += f32::from(line.clearance);
                if line.clearance > gpui::px(0.) || (index == 0 && solid.0 > 0.0) {
                    geometry.cuts.push((solid.0, solid.0));
                }
            }
            let hard: std::collections::HashSet<_> = self
                .opportunities()
                .into_iter()
                .filter(|stop| stop.mandatory)
                .map(|stop| stop.at)
                .collect();
            *node.content.borrow_mut() = geometry
                .solid
                .iter()
                .zip(&self.lines)
                .enumerate()
                .map(
                    |(index, (&(top, bottom), line))| crate::layout::fragment::flow::ContentLine {
                        paragraph: node.ordinal,
                        byte: line.range.start,
                        state: crate::layout::fragment::line_shape::line_state(
                            &self.text,
                            first + index,
                            line.range.start,
                            hard.contains(
                                &self.text[..line.range.start]
                                    .trim_end_matches([' ', '\t'])
                                    .len(),
                            ),
                        )
                        .expect("invalid shaped line address"),
                        top,
                        bottom,
                        prefix: starts[index].1 - growth.get(first + index).copied().unwrap_or(0.0),
                        consumed_before: growth.get(first + index).copied().unwrap_or(0.0),
                    },
                )
                .collect();
            geometry
                .inline
                .push(crate::layout::fragment::flow::inline::Inline {
                    scope: Vec::new(),
                    lines: node.content.borrow().clone(),
                    orphans: keep.0,
                    widows: keep.1,
                });
            geometry.keep_lines(keep.0, keep.1);
            *node.geometry.borrow_mut() = geometry;
        }
    }
}

#[cfg(test)]
mod tests;

impl super::Paragraph {
    pub(crate) fn lay_continuation(
        &self,
        limit: Option<gpui::Pixels>,
        segs: &[super::Seg],
    ) -> Option<Vec<super::Line>> {
        let state = self.fragment.as_ref()?.node.resume.state()?;
        let resume = state.text_pos;
        if !state.past_first_line
            || resume == 0
            || resume >= self.text.len()
            || !self.text.is_char_boundary(resume)
        {
            return None;
        }
        // CSS Break 3 §5.1: продолжается содержимое после прежнего break point.
        // Потреблённый текст не участвует в новом выборе строк и exclusions.
        Some(self.lay_in(resume..self.text.len(), limit, segs))
    }
}

#[cfg(test)]
mod continuation_tests;
