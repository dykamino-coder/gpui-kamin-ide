//! Перенос пустого float в собственном потоке (css-break-3 §2.1).
use super::{Growth, Regrow};
use crate::layout::fragment::flow::{
    FloatGeometry,
    blocks::{BlockBox, Pushed},
};
use crate::tolerance::COORD_EPS;
use std::{collections::HashMap, rc::Rc};

pub(super) fn raise_floor(
    floats: &[FloatGeometry],
    floors: &mut Rc<HashMap<u64, f32>>,
    id: u64,
    amount: f32,
) -> bool {
    let Some(float) = floats.iter().find(|f| f.id == id) else {
        return false;
    };
    let floor = float.anchor + amount;
    let previous = Rc::make_mut(floors).entry(id).or_insert(f32::NEG_INFINITY);
    if floor <= *previous + COORD_EPS {
        return false;
    }
    *previous = floor;
    true
}

/// Разрыв перед первой строкой — разрыв перед float (css-break-3 §4.1).
/// Обе заявки адресуют одну границу margin-box, а не два смещения.
pub(super) fn retarget(
    floats: &[FloatGeometry],
    leads: &[(u64, crate::layout::fragment::flow::LinePush)],
    growth: &mut Growth,
) {
    if let Pushed::Line(line) = &growth.target
        && let Some((id, _)) = leads.iter().find(|(_, head)| head == line)
        && let Some(float) = floats.iter().find(|f| f.id == *id)
    {
        let start = float.top - float.margins[0];
        growth.amount += growth.at - start;
        growth.at = start;
        growth.target = Pushed::Float(*id);
    }
}

impl Regrow {
    pub(crate) fn float_min(&self) -> f32 {
        self.floats
            .iter()
            .map(FloatGeometry::balance_min)
            .fold(0.0, f32::max)
    }

    pub(crate) fn fragment_growths(
        &self,
        blocks: &BlockBox,
        from: f32,
        end: f32,
        edge: f32,
        height: f32,
    ) -> Vec<Growth> {
        let mut grows = super::flex_flow::growths(blocks, &self.laid, from, end, edge);
        grows.extend(self.float_growths(from, edge, height));
        grows
    }

    pub(crate) fn float_growths(&self, from: f32, edge: f32, height: f32) -> Vec<Growth> {
        float_growths(&self.floats, from, edge, height)
    }
}

fn float_growths(floats: &[FloatGeometry], from: f32, edge: f32, height: f32) -> Vec<Growth> {
    floats
        .iter()
        .filter_map(|f| {
            let start = f.top - f.margins[0];
            let end = f.first_break.unwrap_or(f.bottom + f.margins[1]);
            // В начале полного фрагмента avoid уже нельзя выполнить переносом.
            // У первого неполного фрагмента смещается только float, а не parent.
            let at_start = start <= from + COORD_EPS && edge - from >= height - COORD_EPS;
            (start >= from - COORD_EPS
                && start < edge - COORD_EPS
                && end > edge + COORD_EPS
                && !at_start)
                .then_some(Growth {
                    target: Pushed::Float(f.id),
                    at: start,
                    amount: edge - start,
                    forced: false,
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tall_float_uses_its_first_internal_break_before_overflowing() {
        let mut float = FloatGeometry {
            id: 9,
            anchor: 50.0,
            top: 50.0,
            bottom: 250.0,
            margins: [0.0; 2],
            first_break: None,
        };
        assert_eq!(float_growths(&[float], 0.0, 100.0, 100.0)[0].amount, 50.0);
        float.first_break = Some(90.0);
        assert!(float_growths(&[float], 0.0, 100.0, 100.0).is_empty());
        float.first_break = Some(120.0);
        assert_eq!(float_growths(&[float], 0.0, 100.0, 100.0)[0].amount, 50.0);
        float.top = 0.0;
        assert!(float_growths(&[float], 0.0, 100.0, 100.0).is_empty());
    }

    #[test]
    fn float_moves_past_partial_fragment_without_growing_the_parent() {
        let mut float = FloatGeometry {
            id: 7,
            anchor: 0.0,
            top: 0.0,
            bottom: 100.0,
            margins: [0.0; 2],
            first_break: None,
        };
        let grows = float_growths(&[float], 0.0, 50.0, 100.0);
        assert_eq!(grows.len(), 1);
        assert_eq!(grows[0].target, Pushed::Float(7));
        assert_eq!(grows[0].amount, 50.0);
        float.top += grows[0].amount;
        float.bottom += grows[0].amount;
        assert!(float_growths(&[float], 0.0, 50.0, 100.0).is_empty());
        assert!(float_growths(&[float], 50.0, 150.0, 100.0).is_empty());
    }

    #[test]
    fn float_and_first_line_share_a_boundary_after_upstream_growth() {
        use crate::layout::fragment::flow::LinePush;
        let float = FloatGeometry {
            id: 7,
            anchor: 50.,
            top: 150.,
            bottom: 220.,
            margins: [0.; 2],
            first_break: Some(220.),
        };
        let line = LinePush {
            at: 150.,
            line: 0,
            growth: Default::default(),
        };
        let mut growth = Growth {
            target: Pushed::Line(line.clone()),
            at: 150.,
            amount: 50.,
            forced: false,
        };
        retarget(&[float], &[(7, line.clone())], &mut growth);
        assert_eq!(growth.target, Pushed::Float(7));
        assert!(line.growth.borrow().is_empty());
        let mut floors = Rc::default();
        assert!(raise_floor(&[float], &mut floors, 7, growth.amount));
        assert_eq!(floors[&7], 100.);
        // Нормальный поток уже поднял локальный потолок до 100: повторный
        // перенос не должен добавить ещё 50 или увеличить высоту float.
        assert_eq!(75_f32.max(floors[&7]), 100.);
        assert_eq!(100_f32.max(floors[&7]), 100.);
        assert!(!raise_floor(&[float], &mut floors, 7, 50.));
    }
}
