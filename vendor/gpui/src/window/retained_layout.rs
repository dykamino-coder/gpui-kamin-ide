//! Сохранённое дерево независимой раскладки в пределах текущего кадра.

use super::{TaffyLayoutEngine, Window};

/// Цепочка идентификаторов независимо измеренного поддерева в текущем кадре.
/// Геометрией, клипами, стилем текста и rendered view владеет вызывающий код.
#[derive(Clone)]
pub struct ElementIdContext(smallvec::SmallVec<[crate::ElementId; 32]>);

/// KaminIDE patch: a measured subtree keeps its layout IDs through paint.
/// The owner must drop it before the frame arena is reset.
pub struct RetainedLayout(Option<TaffyLayoutEngine>);

impl Default for RetainedLayout {
    fn default() -> Self {
        Self(Some(TaffyLayoutEngine::new()))
    }
}

impl Window {
    /// Сохраняет цепочку до выхода отложенного измерения из своего view.
    pub fn element_id_context(&self) -> ElementIdContext {
        ElementIdContext(self.element_id_stack.clone())
    }

    /// Входит в цепочку подготовленного поддерева без повтора paint-состояния.
    pub fn with_element_id_context<R>(
        &mut self,
        context: &ElementIdContext,
        f: impl FnOnce(&mut Window) -> R,
    ) -> R {
        let outer = std::mem::replace(&mut self.element_id_stack, context.0.clone());
        self.with_restore(
            outer,
            |window, outer| std::mem::swap(&mut window.element_id_stack, outer),
            f,
        )
        .0
    }

    /// Query a registered subtree's intrinsic axis without replacing its final layout.
    pub fn measure_layout_size(
        &mut self,
        id: crate::LayoutId,
        available: crate::Size<crate::AvailableSpace>,
        axis: crate::Axis,
        cx: &mut crate::App,
    ) -> crate::Size<crate::Pixels> {
        let mut engine = self
            .layout_engine
            .take()
            .expect("layout engine during measurement");
        let size = engine.measure_size(id, available, axis, self, cx);
        self.layout_engine = Some(engine);
        size
    }

    /// KaminIDE patch: temporary independent layout for intrinsic queries.
    /// Use `with_retained_layout` when the element must survive until paint.
    pub fn with_nested_layout<R>(&mut self, f: impl FnOnce(&mut Window) -> R) -> R {
        self.with_retained_layout(&mut RetainedLayout::default(), f)
    }

    /// Activate a prepared subtree, restoring both engines even on unwind.
    /// Nested measurement may enter while the surrounding engine is absent.
    pub fn with_retained_layout<R>(
        &mut self,
        retained: &mut RetainedLayout,
        f: impl FnOnce(&mut Window) -> R,
    ) -> R {
        struct Restore<'a> {
            window: &'a mut Window,
            retained: &'a mut RetainedLayout,
            outer: Option<TaffyLayoutEngine>,
        }
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.retained.0 = self.window.layout_engine.take();
                self.window.layout_engine = self.outer.take();
            }
        }
        let outer = self.layout_engine.take();
        self.layout_engine = retained.0.take();
        let restore = Restore {
            window: self,
            retained,
            outer,
        };
        f(restore.window)
    }
}
