//! Каскад: материализованный вычисленный стиль элемента (css-cascade-5 §7.1–7.3). Стадия
//! вычисленных значений (`compute::compute`) идёт по дереву элементов: вычисленный стиль
//! ребёнка считается от ВЫЧИСЛЕННОГО стиля родителя и своего каскадного стиля и запоминается при
//! каскадном стиле элемента (`dom::SharedStyle`). Раскладка, краска и интерактив берут стиль коробки через
//! `box_style`: материализованный стиль плюс контекст коробки-родителя (`inherit::box_stage`).
//!
//! Входы материализации сверяются только по дереву элементов. Копии коробок с другими
//! used-значениями не меняют наследование (CSS 2 §6.2, css-display-3 §2.5). Новый источник
//! каскада передаёт потомкам отдельные computed values, не правки коробки.
//! Размер query container — отдельный вход от адаптера финальной геометрии.
//! Его изменение инвалидирует материализацию и запускает стиль/layout до paint.

use crate::dom::{Node, SharedStyle};
use crate::style::ComputedHandle;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::publication::Published;
use crate::values::ValueCx;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod container_input;
mod inherited;
mod materialized;
pub(crate) use inherited::{inherits_reset, reads_reset_of};
use materialized::{Inputs, Materialized, inputs, stored};
mod recompute;
mod sharing;
mod stage_memo;
mod styled;
pub(crate) use stage_memo::identity;
pub use stage_memo::{OwnKey, StageMemo};
use styled::styled;
mod used;
mod walk;
pub use recompute::{recompute_onto, rematerialize};
use walk::walk;

/// Место материализованного стиля при каскадном стиле элемента (`dom::SharedStyle`): копия
/// элемента делит его с оригиналом, правка каскадного стиля через ручку его сбрасывает.
#[derive(Default)]
pub struct ComputedSlot(
    RefCell<Option<Rc<Materialized>>>,
    Rc<SlotShared>,
    gpui::LayoutInputId,
);

/// What the copies of an element share beside the materialized style: the query container
/// size it was resolved with and the remembered box stages (`stage_memo`). One allocation,
/// made with the slot: a copy made before the first stage reads and fills the same records.
#[derive(Default)]
pub(crate) struct SlotShared {
    container: Cell<(Option<f32>, Option<f32>)>,
    stage: StageMemo,
}

impl Clone for ComputedSlot {
    fn clone(&self) -> Self {
        Self(
            RefCell::new(self.0.borrow().clone()),
            self.1.clone(),
            self.2,
        )
    }
}

impl std::fmt::Debug for ComputedSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.borrow().is_some() {
            "ComputedSlot(materialized)"
        } else {
            "ComputedSlot(empty)"
        })
    }
}

impl ComputedSlot {
    /// Только опубликованная computed-стадия: отсутствие не подменяется каскадом.
    pub fn published(&self) -> Option<ComputedHandle> {
        self.0.borrow().as_ref().map(|m| m.computed.handle.clone())
    }

    /// Материализованный стиль каскадного `own`, годный под родителем `parent`.
    fn get(&self, own: &Computed, parent: &Computed, generation: Inputs) -> Option<Published> {
        self.0
            .borrow()
            .as_ref()
            .filter(|m| {
                m.computed.reset_context.cq_size == self.1.container.get()
                    && m.fits(parent, generation, own)
            })
            .map(|m| m.computed.clone())
    }

    /// Материализованный стиль и вычисленный стиль родителя-элемента, под которым он посчитан.
    fn with_parent(&self) -> Option<(Rc<Computed>, Rc<Computed>)> {
        self.0
            .borrow()
            .as_ref()
            .map(|m| (m.computed.legacy.clone(), m.parent.clone()))
    }

    /// Whether the values of the materialized style read the viewport size.
    pub fn reads_viewport(&self) -> bool {
        (self.0.borrow().as_ref()).is_some_and(|m| m.generation.1.is_some())
    }

    /// Материализованный стиль под родителем-элементом, годен ли он под другим родителем или нет.
    pub fn any(&self) -> Option<Rc<Computed>> {
        self.0.borrow().as_ref().map(|m| m.computed.legacy.clone())
    }

    /// The materialization and the query container size it holds: the computed stage a
    /// remembered box stage was computed from (`stage_memo`).
    pub(crate) fn stage_identity(&self) -> (stage_memo::Identity, (Option<f32>, Option<f32>)) {
        let materialized = self.0.borrow().as_ref().map(stage_memo::identity);
        (materialized, self.1.container.get())
    }

    /// The remembered box stages of this style and its element's copies.
    pub(crate) fn stage_memo(&self) -> &StageMemo {
        &self.1.stage
    }
}

/// Материализовать вычисленные стили элементов `nodes` под вычисленным стилем родителя `parent`
/// и их потомков. Годный стиль не пересчитывается: на неизменном документе проход только
/// сверяет входы.
pub fn materialize(nodes: &[Node], parent: &Rc<Computed>, vcx: ValueCx<'_>) {
    materialize_changed(nodes, parent, vcx);
}

/// Был ли опубликован новый computed style: сигнал для кэшей последующих фаз.
/// Сверка включает viewport, шрифты, наследование и размеры query containers.
pub fn materialize_changed(nodes: &[Node], parent: &Rc<Computed>, vcx: ValueCx<'_>) -> bool {
    crate::work::add!(FullTreeWalks, 1);
    // Записи арен вычисленного стиля живут с документом, а не с кадром.
    let vcx = vcx.persistent();
    walk(nodes, parent, inputs(vcx), &mut Vec::new(), vcx)
}

/// Стиль коробки элемента с каскадным стилем `own` под коробкой-родителем со стилем `parent`.
pub fn box_style(parent: &BoxStyle, own: &SharedStyle, vcx: ValueCx<'_>) -> BoxStyle {
    styled(parent, own, true, vcx)
}

/// То же без окраски цветовым фильтром поддерева (подготовка документа: Filter Effects 1 §5
/// красит цвета, только когда отрисовщик строит элемент).
pub fn box_style_unpainted(parent: &BoxStyle, own: &SharedStyle, vcx: ValueCx<'_>) -> BoxStyle {
    styled(parent, own, false, vcx)
}

#[cfg(test)]
mod format_tests;
#[cfg(test)]
mod paint_tests;
#[cfg(test)]
mod recompute_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tree_tests;
#[cfg(test)]
mod zoom_tests;

#[cfg(test)]
mod publication_tests;

#[cfg(feature = "heap-report")]
impl kamin_heap::HeapSize for ComputedSlot {
    fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        let Ok(slot) = self.0.try_borrow() else {
            return 0;
        };
        let inputs = ops.at("style/materialized", |ops| ops.shallow(Rc::as_ptr(&self.1)))
            + self.1.stage.heap_size(ops);
        let Some(materialized) = slot.as_ref() else {
            return inputs;
        };
        let block = ops.at("style/materialized", |ops| {
            ops.shallow(Rc::as_ptr(materialized))
        });
        if block == 0 {
            return inputs;
        }
        let computed = &materialized.computed;
        inputs
            + block
            + materialized.parent.heap_size(ops)
            + computed.handle.heap_size(ops)
            + computed.legacy.heap_size(ops)
    }
}
