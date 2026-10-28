//! Clamp-контейнер во внешней фрагментации (css-overflow-4 §line-clamp: «clamp containers
//! fragment normally», `line-clamp-fragmentation-001`). Содержимое контейнера раскладывается
//! целиком на его временном движке — точка среза зависит от всех строк его BFC, и счёт строк не
//! начинается заново в следующем фрагментаинере. Внешний рекордер получает один узел хоста:
//! возможности разрыва по строкам и коробкам ПРИМЕНЁННОГО плана до конца коробки. Содержимое за
//! точкой среза скрыто и в поток не входит. Адресов IFC узел не отдаёт: продолжение режет
//! единую раскладку контейнера, а не начинает её счёт с середины.

use crate::layout::fragment::flow::{FlowGeometry, FlowMeasure, FlowNode, Record};
use crate::layout::fragment::geometry::Scope;
use crate::layout::fragment::types::StackAxis;
use crate::tolerance::COORD_EPS;
use gpui::{LayoutId, Pixels, Size, Window, point, px};
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct ClampFlow {
    node: FlowNode,
    content: Rc<FlowMeasure>,
}

impl ClampFlow {
    /// Узел хоста в потоке рекордера `outer`. Поток содержимого принадлежит
    /// только хосту: внешние продолжения и рост строк его не адресуют.
    pub(crate) fn new(outer: Rc<FlowMeasure>, axis: StackAxis) -> Self {
        let node = FlowNode::new(outer, axis);
        let content = Rc::new(FlowMeasure::for_lines());
        Self { node, content }
    }

    /// Поток, в который пишут строки сборки содержимого.
    pub(crate) fn content(&self) -> Rc<FlowMeasure> {
        self.content.clone()
    }

    pub(super) fn record(&self, id: LayoutId, window: &Window) {
        self.node.record(id, window);
    }

    /// Записи одной сборки содержимого: прежние не смешиваются с ней, а
    /// номера абзацев — те же, что у прошлой сборки.
    pub(super) fn scope(&self) -> Scope<Record> {
        self.content.rewind();
        self.content.enter()
    }

    /// Возможности разрыва применённой раскладки с корнем у начала
    /// временного движка и рамочной коробкой `size`.
    pub(super) fn publish(&self, size: Size<Pixels>, window: &mut Window) {
        let geometry = self
            .content
            .at(point(px(0.), px(0.)), size, self.node.axis, window);
        let end = if self.node.axis.is_vertical() {
            size.width
        } else {
            size.height
        };
        *self.node.geometry.borrow_mut() = within(geometry, f32::from(end), self.node.ordinal + 1);
    }
}

/// Содержимое до конца коробки `end`: строки за точкой среза скрыты (§5.3)
/// и разрывов не дают. Строки IFC выбирают разрыв по тем же правилам, что
/// обычный абзац (orphans/widows, css-break-3 §4.4), под адресом `key`
/// хоста; поток содержимого внешнее продолжение не перестраивает — фрагменты
/// режут одну раскладку. Роста строк хост не принимает.
fn within(mut g: FlowGeometry, end: f32, key: usize) -> FlowGeometry {
    let before = |at: f32| at < end - COORD_EPS;
    for p in &mut g.inline {
        p.scope.insert(0, key);
        p.lines.retain(|l| before(l.top));
    }
    g.inline.retain(|p| !p.lines.is_empty());
    for f in &mut g.float_contexts {
        f.scope.insert(0, key);
    }
    g.float_contexts.retain(|f| before(f.bounds.top));
    g.float_leads.clear();
    g.cuts.retain(|&(a, _)| a <= end + COORD_EPS);
    g.solid.retain(|&(a, _)| before(a));
    g.line_pushes.clear();
    g.floats.retain(|f| before(f.top));
    g.boxes.retain(|_, &mut (a, _)| before(a));
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppressed_lines_after_the_box_end_give_no_breaks() {
        let mut g = FlowGeometry::lines([20., 20., 20., 20.]);
        g.boxes.insert(7, (40., 80.));
        g.boxes.insert(8, (0., 40.));
        let kept = within(g, 40., 3);
        assert_eq!(kept.solid, [(0., 20.), (20., 40.)]);
        assert_eq!(kept.cuts, [(20., 20.), (40., 40.)]);
        assert_eq!(kept.boxes.len(), 1);
        assert!(kept.boxes.contains_key(&8));
    }

    #[test]
    fn line_addresses_before_the_end_live_under_the_host_scope() {
        use crate::layout::fragment::flow::{ContentLine, inline::Inline};
        let mut g = FlowGeometry::lines([20., 20., 20.]);
        g.inline.push(Inline {
            scope: vec![2],
            lines: (0..3)
                .map(|i| ContentLine {
                    top: 20. * i as f32,
                    bottom: 20. * (i + 1) as f32,
                    ..Default::default()
                })
                .collect(),
            orphans: 2,
            widows: 2,
        });
        g.inline.push(Inline {
            lines: vec![ContentLine {
                top: 60.,
                bottom: 80.,
                ..Default::default()
            }],
            ..Default::default()
        });
        let kept = within(g, 40., 7);
        assert_eq!(kept.inline.len(), 1);
        assert_eq!(kept.inline[0].scope, [7, 2]);
        assert_eq!(kept.inline[0].lines.len(), 2);
        assert_eq!((kept.inline[0].orphans, kept.inline[0].widows), (2, 2));
    }

    #[test]
    fn exactly_n_lines_keep_every_line_and_the_end_break() {
        let kept = within(FlowGeometry::lines([20., 20.]), 40., 3);
        assert_eq!(kept.solid, [(0., 20.), (20., 40.)]);
        assert_eq!(kept.cuts, [(20., 20.), (40., 40.)]);
    }
}
