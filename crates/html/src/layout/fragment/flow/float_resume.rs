//! Независимый адрес float и его проекция в новый логический exclusion space.
mod project;
use super::{FloatGeometry, FlowGeometry};
use crate::layout::fragment::token::{
    Token,
    fc::{BlockState, FcState},
};

#[derive(Clone, Debug)]
pub(crate) struct Snapshot {
    pub(crate) scope: Vec<usize>,
    pub(crate) bounds: FloatGeometry,
}

impl Snapshot {
    pub(crate) fn translated(&self, offset: f32) -> Self {
        let mut result = self.clone();
        result.bounds.top += offset;
        result.bounds.bottom += offset;
        result.bounds.first_break = result.bounds.first_break.map(|v| v + offset);
        result
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Cursor {
    pub(crate) token: Token,
    pub(crate) boundary: f32,
    addresses: Vec<(Vec<usize>, usize, usize, f32)>,
}

impl Cursor {
    pub(crate) fn at(snapshot: &Snapshot, paragraphs: &[super::inline::Inline], from: f32) -> Self {
        let h = (snapshot.bounds.bottom - snapshot.bounds.top).max(0.0);
        let consumed = (from - snapshot.bounds.top).clamp(0.0, h);
        let mut token = Token::new(FcState::Block(BlockState {
            content_block_size: Some(h),
            ..Default::default()
        }));
        token.consumed = consumed;
        token.at_block_end = consumed >= h;
        let addresses = paragraphs
            .iter()
            .filter(|p| p.scope.starts_with(&snapshot.scope))
            .filter_map(|p| {
                p.lines
                    .iter()
                    .find(|l| l.bottom > from + crate::tolerance::COORD_EPS)
                    .map(|l| {
                        (
                            p.scope[snapshot.scope.len()..].to_vec(),
                            l.paragraph,
                            l.state.text_pos,
                            l.top - snapshot.bounds.top - consumed,
                        )
                    })
            })
            .collect();
        Self {
            token,
            boundary: from - snapshot.bounds.top,
            addresses,
        }
    }

    /// Возвращает сдвиг всего margin-box; края float не обрезаются на разрыве.
    pub(crate) fn shift(&self, fresh: &FlowGeometry, height: f32) -> f32 {
        if self.token.at_block_end {
            return self.token.consumed - height;
        }
        let mapped = self
            .mapped(&fresh.inline, &[])
            .unwrap_or(self.token.consumed.min(height));
        self.token.consumed - mapped
    }

    fn mapped(&self, paragraphs: &[super::inline::Inline], prefix: &[usize]) -> Option<f32> {
        self.addresses
            .iter()
            .filter_map(|(scope, paragraph, byte, delta)| {
                paragraphs
                    .iter()
                    .filter(|p| p.scope.strip_prefix(prefix) == Some(scope.as_slice()))
                    .flat_map(|p| &p.lines)
                    .find(|l| l.paragraph == *paragraph && l.state.text_pos == *byte)
                    .map(|l| l.top - delta)
            })
            .min_by(f32::total_cmp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::flow::{ContentLine, FlowMeasure, inline::Inline};
    use crate::layout::fragment::line_shape::line_state;

    pub(super) fn paragraph(scope: &[usize], step: f32) -> Inline {
        Inline {
            scope: scope.to_vec(),
            lines: (0..8)
                .map(|i| ContentLine {
                    paragraph: 0,
                    byte: i,
                    state: line_state("xxxxxxxx", i, i, false).unwrap(),
                    top: i as f32 * step,
                    bottom: (i + 1) as f32 * step,
                    prefix: i as f32 * step,
                    consumed_before: 0.,
                })
                .collect(),
            orphans: 1,
            widows: 1,
        }
    }
    pub(super) fn snapshot(scope: &[usize], height: f32) -> Snapshot {
        Snapshot {
            scope: scope.to_vec(),
            bounds: FloatGeometry {
                id: 1,
                anchor: 10.,
                top: 0.,
                bottom: height,
                margins: [-10., 20.],
                first_break: Some(10.),
            },
        }
    }

    #[test]
    fn float_maps_its_own_line_and_keeps_the_margin_edges() {
        let old = [paragraph(&[1, 2], 10.), paragraph(&[1, 3], 15.)];
        let cursor = Cursor::at(&snapshot(&[1, 2], 80.), &old, 30.);
        assert_eq!(cursor.token.consumed, 30.);
        let mut fresh = FlowGeometry::default();
        fresh.inline.push(paragraph(&[], 20.));
        assert_eq!(cursor.shift(&fresh, 160.), -30.);
        let shifted = snapshot(&[1, 2], 80.).translated(-30.);
        assert_eq!(shifted.bounds.margins, [-10., 20.]);
        assert_eq!(shifted.bounds.top, -30.);
        assert_eq!(shifted.bounds.first_break, Some(-20.));
    }

    #[test]
    fn nested_scopes_do_not_overwrite_another_float_paragraph_zero() {
        let flow = FlowMeasure::default();
        let paragraphs = [paragraph(&[1, 2], 10.), paragraph(&[1, 3], 15.)];
        flow.resume_paragraphs(&paragraphs, 30.);
        let a = flow.context_path(&[1, 2]).unwrap();
        let b = flow.context_path(&[1, 3]).unwrap();
        assert!(!flow.lines);
        assert!(a.lines && b.lines);
        assert_eq!(a.paragraph_resume(0).get(), 3);
        assert_eq!(b.paragraph_resume(0).get(), 2);
        flow.rewind();
        assert_eq!(a.paragraph_resume(0).get(), 3);
        flow.resume_paragraphs(&[], 0.);
        assert_eq!(a.paragraph_resume(0).get(), 0);
        assert_eq!(b.paragraph_resume(0).get(), 0);
    }

    #[test]
    fn finished_float_keeps_its_old_end_when_its_prefix_reflows() {
        let cursor = Cursor::at(&snapshot(&[1], 80.), &[], 120.);
        assert!(cursor.token.at_block_end);
        assert_eq!(cursor.boundary, 120.);
        assert_eq!(cursor.shift(&FlowGeometry::default(), 160.), -80.);
    }

    #[test]
    fn changed_constraints_keep_consumed_line_space_in_each_float() {
        use crate::layout::fragment::{flow::FlowNode, types::StackAxis};
        let flow = FlowMeasure::for_lines();
        let child = flow.context_path(&[1, 2]).unwrap();
        let old = FlowNode::new(child.clone(), StackAxis::VerticalRl);
        old.growth.borrow_mut().push(12.);
        flow.resume_paragraphs(&[paragraph(&[1, 2], 10.)], 30.);
        let fresh = flow.constraints_only();
        let context = fresh.context_path(&[1, 2]).unwrap();
        let node = FlowNode::new(context.clone(), StackAxis::VerticalRl);
        assert_eq!(*node.growth.borrow(), vec![12.]);
        assert!(std::rc::Rc::ptr_eq(&old.growth, &node.growth));
        assert_eq!(context.paragraph_resume(0).get(), 0);
        assert_eq!(child.paragraph_resume(0).get(), 3);
    }
}
