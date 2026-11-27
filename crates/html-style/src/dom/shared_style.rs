//! Стиль элемента по стадиям значений css-cascade-5 §4: каскадный (выход каскада, его правит только
//! подготовка документа), вычисленный (материализуется из каскадного,
//! `style::cascade::materialize`) и слой used-значений раскладки поверх вычисленного. Раскладка
//! читает вычисленный стиль и пишет только в свой слой: каскадный и вычисленный стили остаются
//! общими для всех копий элемента (Blink: `LayoutObject::Style()` — неизменяемый вычисленный стиль;
//! Servo: `Arc<ComputedValues>`).

use crate::style::ComputedHandle;
use crate::style::box_input::BoxStyleInput;
use crate::style::cascade::materialize::{ComputedSlot, StageMemo};
use crate::style::computed::Computed;
use std::rc::Rc;

/// The style of an element by value stage. Copies of an element share all
/// three stages; an edit copies only the stage it edits.
#[derive(Clone, Debug, Default)]
pub struct SharedStyle {
    /// The cascaded style: cascade output, edited by document preparation only.
    cascaded: Rc<Computed>,
    /// The computed style materialized from `cascaded`.
    computed: ComputedSlot,
    /// Layout's used values: a copy of the computed style that layout edited.
    used: Option<Rc<Computed>>,
    /// Факты коробки в существующем layout-made мосте до A07.
    box_facts: Option<Rc<BoxFacts>>,
    /// Оформление inline в существующем транспорте синтетических коробок.
    decoration: Option<Rc<crate::style::background_paint::DecorationInput>>,
    /// The box stages computed from this style (`materialize::stage_memo`).
    stage: StageMemo,
}

impl From<Computed> for SharedStyle {
    fn from(style: Computed) -> Self {
        Self {
            cascaded: Rc::new(style),
            computed: ComputedSlot::default(),
            used: None,
            box_facts: None,
            decoration: Default::default(),
            stage: StageMemo::default(),
        }
    }
}

// Стиль приходит из каскада уже в куче: не переносить его через кадр обхода.
impl From<Box<Computed>> for SharedStyle {
    fn from(style: Box<Computed>) -> Self {
        Self {
            cascaded: Rc::from(style),
            computed: ComputedSlot::default(),
            used: None,
            box_facts: None,
            decoration: Default::default(),
            stage: StageMemo::default(),
        }
    }
}

impl SharedStyle {
    /// The cascaded style (css-cascade-5 §4.2): input of the computed-value stage.
    pub fn cascaded(&self) -> &Computed {
        &self.cascaded
    }

    /// `vertical-align` of the element's own box (CSS 2.1 §10.8.1; not
    /// inherited): its computed value when the cascade declares one (`inherit`
    /// included, as in the UA rule `td, th { vertical-align: inherit }`), else
    /// the initial `baseline` (`None`). The computed style also hands an inline
    /// ancestor's value down to the text runs inside it, which a box must not
    /// take as its own.
    pub fn own_vertical_align(&self) -> Option<crate::style::computed::Align> {
        let declared = self.cascaded.inh.text_flow.vertical_align.is_some()
            || self
                .cascaded
                .reset_cascade
                .explicit_inherit
                .contains(&crate::style::cascade::inherited_value::Property::VerticalAlign);
        if declared {
            self.used().inh.text_flow.vertical_align
        } else {
            None
        }
    }

    /// Edit the cascaded style (cascade and document preparation, before
    /// materialization): drops computed/used values, retaining the element's
    /// completed container-size input across render-time style copies.
    pub fn cascaded_mut(&mut self) -> &mut Computed {
        self.computed.invalidate();
        self.used = None;
        Rc::make_mut(&mut self.cascaded)
    }

    /// The materialization slot of the cascaded style.
    pub fn slot(&self) -> &ComputedSlot {
        &self.computed
    }

    /// Неизменяемая computed-стадия после материализации. Синтетика без
    /// materialized slot возвращает None, а не каскад или used-копию.
    pub fn published(&self) -> Option<ComputedHandle> {
        self.computed.published()
    }

    /// The element's computed style (css-cascade-5 §4.4): the style materialized
    /// from the cascaded one; layout's used values are not in it.
    ///
    /// TRANSITIONAL(cascade-7b-layout-made): computed() возвращает единственную стадию нематериализованных стилей и layout-made коробок; удалить fallback после отказа от element style handles для синтетических коробок.
    pub fn computed(&self) -> Rc<Computed> {
        self.base()
    }

    /// The style layout reads (css-cascade-5 §4.5): the computed style with
    /// layout's used values (`used_mut`) laid over it.
    pub fn used(&self) -> BoxStyleInput {
        let style = match &self.used {
            Some(used) => used.clone(),
            None => self.base(),
        };
        let facts = self.box_facts.as_deref();
        BoxStyleInput {
            try_source: facts.and_then(|f| f.try_source.clone()),
            decor_box: self.decor_box(),
            flow_align: self.flow_align(),
            tree: facts.map(|f| f.tree).unwrap_or_default(),
            decors: self
                .decoration
                .as_deref()
                .map(|d| d.decors.clone())
                .unwrap_or_default(),
            ..BoxStyleInput::new(
                style,
                facts.and_then(|f| f.context.clone()),
                self.decoration
                    .as_deref()
                    .map(|d| d.inline)
                    .unwrap_or_default(),
                self.decoration
                    .as_deref()
                    .map(|d| d.background_paint)
                    .unwrap_or_default(),
                self.decoration.as_deref().and_then(|d| d.canvas.clone()),
                self.decoration.as_deref().and_then(|d| d.paint.clone()),
            )
        }
    }

    /// Identity of the decorating box (css-text-decor-3 §2.1): only the cascaded stage has it.
    pub fn decor_box(&self) -> u64 {
        self.box_facts
            .as_deref()
            .and_then(|f| f.decor_box)
            .unwrap_or(self.cascaded.decor_box)
    }

    /// Только собственный фон; потомки наследуют computed CSS, а не этот вход.
    pub fn set_background_paint(&mut self, paint: crate::style::background_paint::BackgroundPaint) {
        let mut decoration = self.decoration.as_deref().cloned().unwrap_or_default();
        decoration.background_paint = paint;
        self.decoration = Some(Rc::new(decoration)).filter(|d| **d != Default::default());
    }

    /// Собственный источник холста, не computed CSS корня и не наследуемый фон.
    pub fn set_canvas_background(
        &mut self,
        canvas: Option<crate::style::canvas_background::CanvasBackgroundSource>,
    ) {
        let mut decoration = self.decoration.as_deref().cloned().unwrap_or_default();
        decoration.canvas = canvas;
        self.decoration = Some(Rc::new(decoration)).filter(|d| **d != Default::default());
    }

    /// The style of an element whose box layout generated from a box style (a
    /// `::marker` box, the `foreignObject` viewport root): its values are
    /// computed already and become its only stage. Moved element boxes keep
    /// their own style (`Element::set_moved_box`).
    ///
    /// TRANSITIONAL(cascade-7b-layout-made): layout_made() выдаёт computed box values за cascade при повторном входе через Element; удалить после сохранения исходного style handle и отдельной передачи BoxStyle.
    pub fn layout_made(input: impl Into<BoxStyleInput>) -> Self {
        let input: BoxStyleInput = input.into();
        let facts = BoxFacts {
            context: input.context,
            try_source: input.try_source,
            moved_from: None,
            decor_box: (input.decor_box != input.style.decor_box).then_some(input.decor_box),
            flow_align: input.flow_align,
            tree: input.tree,
        };
        let decoration = crate::style::background_paint::DecorationInput {
            inline: input.inline_decoration,
            background_paint: input.background_paint,
            canvas: input.canvas_background,
            paint: input.paint,
            decors: input.decors,
        };
        Self {
            cascaded: input.style,
            computed: ComputedSlot::default(),
            used: None,
            box_facts: (facts.context.is_some()
                || facts.try_source.is_some()
                || facts.decor_box.is_some()
                || facts.flow_align.is_some()
                || facts.tree != Default::default())
            .then(|| Rc::new(facts)),
            decoration: Some(Rc::new(decoration)).filter(|d| **d != Default::default()),
            stage: StageMemo::default(),
        }
    }

    /// Layout's used-value layer, copy-on-write over the computed style.
    ///
    /// TRANSITIONAL(cascade-7b-layout-made): used_mut() меняет единственную стадию нематериализованного стиля; удалить fallback после разделения стадий у всех синтетических коробок.
    pub fn used_mut(&mut self) -> &mut Computed {
        if self.used.is_none() && self.computed.any().is_none() {
            return Rc::make_mut(&mut self.cascaded);
        }
        let base = match self.used.take() {
            Some(used) => used,
            None => self.base(),
        };
        Rc::make_mut(self.used.insert(base))
    }

    /// Layout's used-value layer, if layout edited this style.
    pub fn used_rc(&self) -> Option<Rc<Computed>> {
        self.used.clone()
    }

    fn base(&self) -> Rc<Computed> {
        self.computed.any().unwrap_or_else(|| self.cascaded.clone())
    }
}

mod derive;
mod facts;
#[cfg(feature = "heap-report")]
mod heap;
use facts::BoxFacts;
mod restyle;
mod same;
mod shared_used;
mod stage;
#[cfg(test)]
mod tests;
