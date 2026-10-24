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

use crate::document::state::ValueCx;
use crate::dom::{Node, SharedStyle};
use crate::style::computed::{BoxStyle, Computed};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod container_input;
mod inherited;
mod recompute;
mod tree;
mod used;
pub(crate) use recompute::{recompute_onto, rematerialize};

/// Место материализованного стиля при каскадном стиле элемента (`dom::SharedStyle`): копия
/// элемента делит его с оригиналом, правка каскадного стиля через ручку его сбрасывает.
#[derive(Default)]
pub struct ComputedSlot(
    RefCell<Option<Rc<Materialized>>>,
    Rc<Cell<(Option<f32>, Option<f32>)>>,
    gpui::LayoutInputId,
);

type Inputs = (u64, [u32; 2]);

fn inputs(vcx: ValueCx<'_>) -> Inputs {
    (
        vcx.doc.metrics.generation(),
        [vcx.viewport.0.to_bits(), vcx.viewport.1.to_bits()],
    )
}

struct Materialized {
    /// Вычисленный стиль родителя, под которым он посчитан.
    parent: Rc<Computed>,
    /// Поколение шрифтовых входов и размеры viewport для относительных единиц.
    generation: Inputs,
    computed: Rc<Computed>,
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

impl Materialized {
    /// Годится ли стиль под родителем `parent`: те же унаследованные данные. Стиль с
    /// каскадным `inherit` ненаследуемого свойства (`explicit`) зависит и от
    /// ненаследуемых значений родителя (css-cascade-5 §7.3.1): ему годен только тот же
    /// родитель или равный во всём (Blink `MatchedPropertiesCache`: explicit inheritance
    /// сверяет и non-inherited данные родителя).
    fn fits(&self, parent: &Computed, generation: Inputs, explicit: bool) -> bool {
        self.generation == generation
            && (std::ptr::eq(&*self.parent, parent)
                || if explicit {
                    *self.parent == *parent
                } else {
                    inherited::inherited_eq(&self.parent, parent)
                })
    }
}

impl ComputedSlot {
    /// Материализованный стиль каскадного `own`, годный под родителем `parent`.
    fn get(&self, own: &Computed, parent: &Computed, generation: Inputs) -> Option<Rc<Computed>> {
        let explicit = !own.explicit_inherit.is_empty() || !own.inherit_longhands.is_empty();
        self.0
            .borrow()
            .as_ref()
            .filter(|m| m.computed.cq_size == self.1.get() && m.fits(parent, generation, explicit))
            .map(|m| m.computed.clone())
    }

    /// Материализованный стиль и вычисленный стиль родителя-элемента, под которым он посчитан.
    fn with_parent(&self) -> Option<(Rc<Computed>, Rc<Computed>)> {
        self.0
            .borrow()
            .as_ref()
            .map(|m| (m.computed.clone(), m.parent.clone()))
    }

    /// Материализованный стиль под родителем-элементом, годен ли он под другим родителем или нет.
    pub(crate) fn any(&self) -> Option<Rc<Computed>> {
        self.0.borrow().as_ref().map(|m| m.computed.clone())
    }
}

/// Материализовать вычисленные стили элементов `nodes` под вычисленным стилем родителя `parent`
/// и их потомков. Годный стиль не пересчитывается: на неизменном документе проход только
/// сверяет входы.
pub(crate) fn materialize(nodes: &[Node], parent: &Rc<Computed>, vcx: ValueCx<'_>) {
    // Записи арен вычисленного стиля живут с документом, а не с кадром.
    let vcx = vcx.persistent();
    let generation = inputs(vcx);
    tree::walk(nodes, parent, generation, &mut Vec::new(), vcx);
}

/// Стиль коробки элемента с каскадным стилем `own` под коробкой-родителем со стилем `parent`.
pub fn box_style(parent: &BoxStyle, own: &SharedStyle, vcx: ValueCx<'_>) -> BoxStyle {
    styled(parent, own, true, vcx)
}

/// То же без окраски цветовым фильтром поддерева (подготовка документа: Filter Effects 1 §5
/// красит цвета, только когда отрисовщик строит элемент).
pub(crate) fn box_style_unpainted(
    parent: &BoxStyle,
    own: &SharedStyle,
    vcx: ValueCx<'_>,
) -> BoxStyle {
    styled(parent, own, false, vcx)
}

fn styled(parent: &BoxStyle, own: &SharedStyle, paint: bool, vcx: ValueCx<'_>) -> BoxStyle {
    let cascaded = own.cascaded();
    let element = own.slot().with_parent().map(|(original, element_parent)| {
        // Только новая computed-стадия родителя-элемента (например, источник
        // position-try) требует вычислить ребёнка заново. Used-копия её не меняет.
        let context = parent.element_style.as_ref();
        let computed = context
            .and_then(|p| p.variant_for(&element_parent))
            .map(|p| own.slot().compute(p, cascaded, vcx))
            .unwrap_or_else(|| original.clone());
        crate::style::computed::ElementStyle {
            original,
            computed,
            parent: context.and_then(|p| p.active()),
        }
    });
    let computed = match &element {
        Some(e) => match own.used_rc() {
            Some(used) if Rc::ptr_eq(&e.original, &e.computed) => used,
            Some(used) => {
                let mut c = (*e.computed).clone();
                used::lay_used(&mut c, &used, &e.original);
                Rc::new(c)
            }
            None => e.computed.clone(),
        },
        // Анонимная коробка наследует от своей коробки-родителя.
        None => Rc::new(super::compute::compute(parent, cascaded, vcx)),
    };
    let view = own.used();
    // Заданы ли цвета — по каскаду; у стиля без материализации стадия одна.
    let declared = match own.slot().any() {
        Some(_) => cascaded,
        None => &*view,
    };
    let mut boxed =
        super::inherit::box_stage(parent, declared, &view, computed, element, paint, vcx);
    if !boxed.position_try_fallbacks.is_empty() {
        boxed.try_style = Some(own.clone());
    }
    boxed
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
