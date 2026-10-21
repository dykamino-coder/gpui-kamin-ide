//! Пробы prepaint, читаемые в paint того же кадра. Для геометрии раскладки
//! нужен замер внутри хоста, а не перенос результатов Sink на следующий кадр.

use gpui::{AnyElement, Bounds, IntoElement, Pixels, Styled};
use std::{cell::RefCell, rc::Rc};

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
impl<T: 'static> Sink<T> {
    pub(crate) fn push(&self, value: T) {
        self.0.borrow_mut().push(value);
    }

    pub(crate) fn record(&self, f: impl FnOnce(Bounds<Pixels>) -> T + 'static) -> AnyElement {
        let sink = self.clone();
        gpui::canvas(move |bounds, _, _| sink.push(f(bounds)), |_, _, _, _| {})
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .into_any_element()
    }
    pub(crate) fn take(&self) -> Vec<T> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
    #[cfg(test)]
    pub(crate) fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Проба раскладки: запись снимается до уничтожения временного layout-движка.
pub(crate) fn layout_record<T: 'static>(sink: Sink<(gpui::LayoutId, T)>, value: T) -> AnyElement {
    use crate::hooks::IntoHooked;
    LayoutRecord {
        sink,
        value: Some(value),
    }
    .hooked()
}

struct LayoutRecord<T> {
    sink: Sink<(gpui::LayoutId, T)>,
    value: Option<T>,
}

impl<T: 'static> crate::hooks::Hooks for LayoutRecord<T> {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, ()) {
        let mut style = gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        style.inset.top = gpui::px(0.).into();
        style.inset.left = gpui::px(0.).into();
        style.size = gpui::size(gpui::relative(1.).into(), gpui::relative(1.).into());
        let id = window.request_layout(style, [], cx);
        if let Some(value) = self.value.take() {
            self.sink.push((id, value));
        }
        (id, ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut gpui::Window, _: &mut gpui::App) {
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        _: &mut gpui::Window,
        _: &mut gpui::App,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::Sink;

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
