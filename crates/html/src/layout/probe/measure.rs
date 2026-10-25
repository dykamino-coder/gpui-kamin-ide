//! Записи LayoutId живут только в области текущего движка; наружу выходят числа.

use super::{Sink, full_box};
use crate::hooks::{Hooks, IntoHooked};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};

#[derive(Debug)]
pub(crate) struct Measure<T>(Sink<(LayoutId, T)>);
impl<T> Clone for Measure<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> Default for Measure<T> {
    fn default() -> Self {
        Self(Sink::default())
    }
}
impl<T> Measure<T> {
    pub(crate) fn record(&self, id: LayoutId, value: T) {
        self.0.push((id, value));
    }
    pub(crate) fn enter(&self) -> MeasureScope<T> {
        MeasureScope {
            owner: self.clone(),
            saved: self.0.take(),
        }
    }
    pub(crate) fn take(&self, window: &mut Window) -> Vec<(Bounds<Pixels>, T)> {
        self.0
            .take()
            .into_iter()
            .map(|(id, value)| (bounds(id, window), value))
            .collect()
    }
}
impl<T: 'static> Measure<T> {
    pub(crate) fn probe(&self, value: T) -> AnyElement {
        LayoutRecord {
            measure: self.clone(),
            value: Some(value),
        }
        .hooked()
    }
}
fn bounds(id: LayoutId, window: &mut Window) -> Bounds<Pixels> {
    Bounds {
        origin: window.layout_origin_unrounded(id),
        size: window.layout_size_unrounded(id),
    }
}
pub(crate) struct MeasureScope<T> {
    owner: Measure<T>,
    saved: Vec<(LayoutId, T)>,
}
impl<T> Drop for MeasureScope<T> {
    fn drop(&mut self) {
        *self.owner.0.0.borrow_mut() = std::mem::take(&mut self.saved);
    }
}
struct LayoutRecord<T> {
    measure: Measure<T>,
    value: Option<T>,
}
impl<T: 'static> Hooks for LayoutRecord<T> {
    type Layout = ();
    type Prepaint = ();
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let id = window.request_layout(full_box(), [], cx);
        if let Some(value) = self.value.take() {
            self.measure.record(id, value);
        }
        (id, ())
    }
    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}
    fn paint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut (), _: &mut Window, _: &mut App) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_layout_scopes_discard_inner_ids_and_restore_outer_records() {
        let a = LayoutId::from(taffy::NodeId::from(1_u64));
        let measure = Measure::default();
        measure.record(a, "outer");
        {
            let _scope = measure.enter();
            assert!(measure.0.borrow().is_empty());
            // Временный движок вправе повторно выдать тот же идентификатор.
            let b = a;
            measure.record(b, "inner");
            {
                let _scope = measure.enter();
                measure.record(b, "innermost");
            }
            assert_eq!(*measure.0.borrow(), [(b, "inner")]);
        }
        assert_eq!(*measure.0.borrow(), [(a, "outer")]);
    }
}
