//! Каскад: материализованный вычисленный стиль элемента (css-cascade-5 §7.1–7.3). Стадия
//! вычисленных значений (`compute::compute`) идёт по дереву элементов один раз: вычисленный стиль
//! ребёнка считается от ВЫЧИСЛЕННОГО стиля родителя и своего каскадного стиля и запоминается при
//! каскадном стиле элемента (`dom::SharedStyle`). Раскладка, краска и интерактив берут стиль коробки через
//! `box_style`: материализованный стиль плюс контекст коробки-родителя (`inherit::box_stage`).
//!
//! Материализованный стиль годится, пока совпадают его входы (Blink: `InheritedEqual` решает,
//! пересчитывать ли детей; Stylo: кэш правил по родителю): тот же каскадный стиль (правка
//! раскладкой через ручку сбрасывает материализованный), те же унаследованные данные родителя
//! и те же шрифтовые входы документа. Иначе — родитель коробки отличается от родителя-элемента
//! (слой `::first-line`, кадр анимации, положенная плашмя копия вертикального абзаца, окраска
//! поддерева фильтром) или это анонимная коробка — стиль считается от того родителя, что дан.

use crate::document::state::ValueCx;
use crate::dom::{Node, SharedStyle};
use crate::style::computed::Computed;
use std::cell::RefCell;
use std::rc::Rc;

mod inherited;
mod used;

/// Место материализованного стиля при каскадном стиле элемента (`dom::SharedStyle`): копия
/// элемента делит его с оригиналом, правка каскадного стиля через ручку его сбрасывает.
#[derive(Default)]
pub struct ComputedSlot(RefCell<Option<Rc<Materialized>>>);

struct Materialized {
    /// Вычисленный стиль родителя, под которым он посчитан.
    parent: Rc<Computed>,
    /// Поколение шрифтовых входов документа (`text::metrics`).
    generation: u64,
    computed: Rc<Computed>,
}

impl Clone for ComputedSlot {
    fn clone(&self) -> Self {
        Self(RefCell::new(self.0.borrow().clone()))
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
    /// Годится ли стиль под родителем `parent`: каскадное `inherit` ненаследуемого свойства
    /// читает у родителя и его ненаследуемые поля — тогда годен только тот же самый родитель.
    fn fits(&self, own: &Computed, parent: &Computed, generation: u64) -> bool {
        self.generation == generation
            && (std::ptr::eq(&*self.parent, parent)
                || !inherited::inherits_explicitly(own)
                    && inherited::inherited_eq(&self.parent, parent))
    }
}

impl ComputedSlot {
    /// Материализованный стиль, годный под родителем `parent`.
    fn get(&self, own: &Computed, parent: &Computed, generation: u64) -> Option<Rc<Computed>> {
        self.0
            .borrow()
            .as_ref()
            .filter(|m| m.fits(own, parent, generation))
            .map(|m| m.computed.clone())
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
    let generation = vcx.doc.metrics.generation();
    for node in nodes {
        let Node::Element(e) = node else { continue };
        let slot = e.style.slot();
        let computed = match slot.get(e.style.cascaded(), parent, generation) {
            Some(computed) => computed,
            None => {
                let computed = Rc::new(super::compute::compute(parent, e.style.cascaded(), vcx));
                *slot.0.borrow_mut() = Some(Rc::new(Materialized {
                    parent: parent.clone(),
                    generation,
                    computed: computed.clone(),
                }));
                computed
            }
        };
        materialize(&e.children, &computed, vcx);
    }
}

/// Стиль коробки элемента с каскадным стилем `own` под коробкой-родителем со стилем `parent`.
pub fn box_style(parent: &Computed, own: &SharedStyle, vcx: ValueCx<'_>) -> Computed {
    styled(parent, own, true, vcx)
}

/// То же без окраски цветовым фильтром поддерева (подготовка документа: Filter Effects 1 §5
/// красит цвета, только когда отрисовщик строит элемент).
pub(crate) fn box_style_unpainted(
    parent: &Computed,
    own: &SharedStyle,
    vcx: ValueCx<'_>,
) -> Computed {
    styled(parent, own, false, vcx)
}

fn styled(parent: &Computed, own: &SharedStyle, paint: bool, vcx: ValueCx<'_>) -> Computed {
    let generation = vcx.doc.metrics.generation();
    let cascaded = own.cascaded();
    let computed = match (own.slot().get(cascaded, parent, generation), own.used()) {
        // Годный материализованный стиль: слой раскладки уже лежит поверх него.
        (Some(_), Some(used)) => used.clone(),
        (Some(computed), None) => (*computed).clone(),
        (None, used) => match (own.slot().any(), used) {
            // Другой родитель: стиль под ним и поля, которые поставила раскладка.
            (Some(materialized), Some(used)) => {
                let mut c = super::compute::compute(parent, cascaded, vcx);
                used::lay_used(&mut c, used, &materialized);
                c
            }
            // Стиль коробки, которую сделала раскладка: его значения — её.
            (None, Some(used)) => super::compute::compute(parent, used, vcx),
            (_, None) => super::compute::compute(parent, cascaded, vcx),
        },
    };
    let view = own.used().unwrap_or(cascaded);
    // Заданы ли цвета — по каскаду; у коробки раскладки каскада нет.
    let declared = match own.slot().any() {
        Some(_) => cascaded,
        None => view,
    };
    super::inherit::box_stage(parent, declared, view, computed, paint, vcx)
}

#[cfg(test)]
mod tests;
