//! Сохранённое дерево независимой раскладки в пределах текущего кадра.

use super::{TaffyLayoutEngine, Window};
use crate::ElementId;
use smallvec::SmallVec;
use std::{cell::RefCell, rc::Rc};

/// Цепочка идентификаторов независимо измеренного поддерева в текущем кадре.
/// Геометрией, клипами, стилем текста и rendered view владеет вызывающий код.
/// Снимок хранит только живых предков; копии разделяют неизменяемый срез.
#[derive(Clone)]
pub struct ElementIdContext(Rc<[crate::ElementId]>);

/// Inputs inherited by an element constructed during deferred measurement.
/// Paint geometry, clips and effects remain owned by the current traversal.
#[derive(Clone)]
pub struct ElementLayoutContext {
    identity: ElementIdContext,
    /// Inherited text style and rem size: the scope every measure callback
    /// already runs in (`Window::measure_scope`).
    scope: super::text_style_scope::MeasureScope,
}

/// KaminIDE patch: a measured subtree keeps its layout IDs through paint.
/// The owner must drop it before the frame arena is reset.
#[derive(Clone)]
pub struct RetainedLayout(Rc<RefCell<Option<TaffyLayoutEngine>>>);

impl Default for RetainedLayout {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(Some(
            TaffyLayoutEngine::new_for_subtree(),
        ))))
    }
}

impl RetainedLayout {
    pub(super) fn active() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }
}

impl Window {
    /// Удерживает входы отложенного измерения до завершения потомков.
    pub async fn with_element_layout_context_async<R>(
        &mut self,
        context: &ElementLayoutContext,
        f: impl AsyncFnOnce(&mut Window) -> R,
    ) -> R {
        context
            .scope
            .enter_async(self, async |window| {
                window
                    .with_element_id_context_async(&context.identity, f)
                    .await
            })
            .await
    }
    /// Retain the arena owning the current traversal's layout IDs.
    /// Deferred prepaint and paint must reactivate it before using those IDs.
    pub fn retained_layout(&self) -> RetainedLayout {
        self.layout_owner.clone()
    }

    /// Сохраняет цепочку до выхода отложенного измерения из своего view.
    pub fn element_id_context(&self) -> ElementIdContext {
        ElementIdContext(Rc::from(self.element_id_stack.as_slice()))
    }

    /// Preserve inherited layout inputs before a measured callback leaves its ancestors.
    pub fn element_layout_context(&self) -> ElementLayoutContext {
        ElementLayoutContext {
            identity: self.element_id_context(),
            scope: self.measure_scope(),
        }
    }

    /// Construct a measured subtree with the inputs of its original ancestors.
    pub fn with_element_layout_context<R>(
        &mut self,
        context: &ElementLayoutContext,
        f: impl FnOnce(&mut Window) -> R,
    ) -> R {
        context.scope.enter(self, |window| {
            window.with_element_id_context(&context.identity, f)
        })
    }

    /// Входит в цепочку подготовленного поддерева без повтора paint-состояния.
    pub fn with_element_id_context<R>(
        &mut self,
        context: &ElementIdContext,
        f: impl FnOnce(&mut Window) -> R,
    ) -> R {
        let outer = self.replace_element_id_context(context);
        self.with_restore(
            outer,
            |window, outer| std::mem::swap(&mut window.element_id_stack, outer),
            f,
        )
        .0
    }

    pub(super) fn replace_element_id_context(
        &mut self,
        context: &ElementIdContext,
    ) -> SmallVec<[ElementId; 32]> {
        std::mem::replace(
            &mut self.element_id_stack,
            context.0.iter().cloned().collect(),
        )
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

    /// Независимое измерение сохраняет временную арену через приостановки.
    pub async fn with_nested_layout_async<R>(
        &mut self,
        f: impl AsyncFnOnce(&mut Window) -> R,
    ) -> R {
        self.with_retained_layout_async(&mut RetainedLayout::default(), f).await
    }

    /// Activate a prepared subtree, restoring both engines even on unwind.
    /// Nested measurement may enter while the surrounding engine is absent.
    pub fn with_retained_layout<R>(
        &mut self,
        retained: &mut RetainedLayout,
        f: impl FnOnce(&mut Window) -> R,
    ) -> R {
        let outer = self.activate_retained_layout(retained);
        self.with_restore(outer, Self::restore_retained_layout, f).0
    }

    pub(super) fn activate_retained_layout(
        &mut self,
        retained: &RetainedLayout,
    ) -> Option<RetainedLayout> {
        if Rc::ptr_eq(&retained.0, &self.layout_owner.0) {
            return None;
        }
        let outer = std::mem::replace(&mut self.layout_owner, retained.clone());
        *outer.0.borrow_mut() = self.layout_engine.take();
        self.layout_engine = retained.0.borrow_mut().take();
        Some(outer)
    }

    pub(super) fn restore_retained_layout(&mut self, outer: &mut Option<RetainedLayout>) {
        if let Some(outer) = outer {
            *self.layout_owner.0.borrow_mut() = self.layout_engine.take();
            self.layout_engine = outer.0.borrow_mut().take();
            self.layout_owner = outer.clone();
        }
    }
}
