//! Пробы одного кадра: Sink читает геометрию в prepaint, Measure — внутри layout-хоста.
//! LayoutId не переносится через границу временного движка; paint читает только значения.

mod measure;
mod record;
use gpui::{AnyElement, Bounds, Pixels, Window};
pub(crate) use measure::{Measure, MeasureScope};
pub(crate) use record::{Geometry, full_box};
use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

#[derive(Debug)]
pub struct Sink<T>(Rc<RefCell<Vec<T>>>);

impl<T> Clone for Sink<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> Default for Sink<T> {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(Vec::new())))
    }
}
impl<T> Sink<T> {
    pub(crate) fn push(&self, value: T) {
        self.0.borrow_mut().push(value);
    }
    pub(crate) fn take(&self) -> Vec<T> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
    pub(crate) fn fork(&self) -> Self
    where
        T: Clone,
    {
        Self(Rc::new(RefCell::new(self.borrow().clone())))
    }
    pub(crate) fn borrow(&self) -> Ref<'_, Vec<T>> {
        self.0.borrow()
    }
}
impl<T: 'static> Sink<T> {
    /// Все записи завершаются до paint, включая записи вложенных OOF-хостов.
    pub(crate) fn record(&self, f: impl FnOnce(Bounds<Pixels>) -> T + 'static) -> AnyElement {
        self.record_with(move |g, _| f(g.bounds))
    }
    pub(crate) fn record_with(
        &self,
        f: impl FnOnce(Geometry, &mut Window) -> T + 'static,
    ) -> AnyElement {
        self.record_styled(full_box(), f)
    }
    pub(crate) fn record_styled(
        &self,
        style: gpui::Style,
        f: impl FnOnce(Geometry, &mut Window) -> T + 'static,
    ) -> AnyElement {
        record::prepaint(self.clone(), style, f)
    }
}

#[cfg(test)]
mod tests {
    use super::Sink;
    #[test]
    fn candidate_forks_keep_prior_geometry_but_never_publish_rejected_records() {
        let live = Sink::default();
        live.push((1, "parent"));
        let candidate = live.fork();
        candidate.push((2, "rejected"));
        live.push((3, "selected"));
        assert_eq!(candidate.take(), [(1, "parent"), (2, "rejected")]);
        assert_eq!(live.take(), [(1, "parent"), (3, "selected")]);
    }

    #[test]
    fn taking_a_probe_batch_keeps_clones_connected() {
        let sink = Sink::default();
        let probe = sink.clone();
        probe.push(1);
        sink.push(2);
        assert_eq!(sink.take(), [1, 2]);
        assert!(probe.take().is_empty());
        probe.push(3);
        assert_eq!(sink.take(), [3]);
    }
}
