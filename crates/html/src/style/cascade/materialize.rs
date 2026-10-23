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
use crate::style::computed::{BoxStyle, Computed};
use std::cell::RefCell;
use std::rc::Rc;

mod inherited;
mod recompute;
mod used;
pub(crate) use recompute::{recompute_onto, rematerialize};

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
    /// Коробку элемента подготовка документа перенесла под чужую коробку
    /// (`Marks::style_parent`): коробка-родитель не источник его наследования.
    moved: bool,
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
    /// Годится ли стиль под родителем `parent`: те же унаследованные данные. Стиль с
    /// каскадным `inherit` ненаследуемого свойства (`explicit`) зависит и от
    /// ненаследуемых значений родителя (css-cascade-5 §7.3.1): ему годен только тот же
    /// родитель или равный во всём (Blink `MatchedPropertiesCache`: explicit inheritance
    /// сверяет и non-inherited данные родителя).
    fn fits(&self, parent: &Computed, generation: u64, explicit: bool) -> bool {
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
    fn get(&self, own: &Computed, parent: &Computed, generation: u64) -> Option<Rc<Computed>> {
        let explicit = !own.explicit_inherit.is_empty() || !own.inherit_longhands.is_empty();
        self.0
            .borrow()
            .as_ref()
            .filter(|m| m.fits(parent, generation, explicit))
            .map(|m| m.computed.clone())
    }

    /// Материализованный стиль перенесённой коробки (`Materialized::moved`): он годен под
    /// любой коробкой-родителем, пока те же шрифтовые входы.
    fn moved(&self, generation: u64) -> Option<Rc<Computed>> {
        self.0
            .borrow()
            .as_ref()
            .filter(|m| m.moved && m.generation == generation)
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
    let generation = vcx.doc.metrics.generation();
    walk(nodes, parent, generation, &mut Vec::new(), vcx);
}

/// Проход `materialize`. Родитель-элемент — не обязательно узел над элементом в дереве
/// коробок: анонимная обёртка (`Marks::anonymous_wrapper`) пропускает родителя насквозь,
/// перенесённая коробка (`Marks::style_parent`) берёт стиль исходного родителя, который
/// проход уже встретил раньше по порядку документа (`lent`).
fn walk(
    nodes: &[Node],
    parent: &Rc<Computed>,
    generation: u64,
    lent: &mut Vec<(u64, Rc<Computed>)>,
    vcx: ValueCx<'_>,
) {
    for node in nodes {
        let Node::Element(e) = node else { continue };
        let moved = e.marks.style_parent;
        // Вне прохода по документу (`rematerialize`) родитель дан уже исходный.
        let source = moved.and_then(|id| lent.iter().rev().find(|(k, _)| *k == id));
        let parent = source
            .map(|(_, c)| c.clone())
            .unwrap_or_else(|| parent.clone());
        let slot = e.style.slot();
        let computed = match slot.get(e.style.cascaded(), &parent, generation) {
            Some(computed) => computed,
            None => {
                let computed = Rc::new(super::compute::compute(&parent, e.style.cascaded(), vcx));
                *slot.0.borrow_mut() = Some(Rc::new(Materialized {
                    parent: parent.clone(),
                    generation,
                    computed: computed.clone(),
                    moved: moved.is_some(),
                }));
                computed
            }
        };
        if e.marks.lends_style {
            lent.push((e.node_id, computed.clone()));
        }
        let inside = if e.marks.anonymous_wrapper {
            &parent
        } else {
            &computed
        };
        walk(&e.children, inside, generation, lent, vcx);
    }
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
    let generation = vcx.doc.metrics.generation();
    let cascaded = own.cascaded();
    // Выбранный вариант `@position-try` — источник каскада родителя (Position
    // Fallback Origin, css-anchor-position-1 §5.1): его объявления входят в
    // вычисленные значения родителя-элемента, а их знает только коробка
    // родителя. `inherit` берёт их оттуда (`inherit-height-from-fallback`).
    let fallback_parent = parent.try_base.is_some();
    // Перенесённая коробка наследует от родителя-элемента, а не от коробки, под
    // которую её перенесли.
    let fitting = own
        .slot()
        .get(cascaded, parent, generation)
        .or_else(|| own.slot().moved(generation))
        .filter(|_| !fallback_parent);
    // Годный материализованный стиль (со слоем раскладки) — общий, без копии (P-18).
    let computed = match (fitting, own.used_rc()) {
        (Some(_), Some(used)) => used,
        (Some(computed), None) => computed,
        (None, used) => match own.slot().with_parent() {
            // Другие унаследованные данные (`::first-line`, положенная плашмя
            // копия абзаца): наследуемое — от коробки-родителя, каскадное
            // `inherit` — от родителя-элемента; поверх — поля раскладки.
            Some((materialized, element_parent)) => {
                let mut from = if fallback_parent {
                    Computed::clone(parent)
                } else {
                    (*element_parent).clone()
                };
                inherited::take_inherited(&mut from, parent);
                let mut c = super::compute::compute(&from, cascaded, vcx);
                if let Some(used) = used {
                    used::lay_used(&mut c, &used, &materialized);
                }
                Rc::new(c)
            }
            // Стиль одной стадии: подготовка документа, коробка раскладки.
            None => Rc::new(super::compute::compute(parent, cascaded, vcx)),
        },
    };
    let view = own.used();
    // Заданы ли цвета — по каскаду; у стиля без материализации стадия одна.
    let declared = match own.slot().any() {
        Some(_) => cascaded,
        None => &*view,
    };
    super::inherit::box_stage(parent, declared, &view, computed, paint, vcx)
}

#[cfg(test)]
mod recompute_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tree_tests;
