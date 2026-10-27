//! Стиль элемента по стадиям значений css-cascade-5 §4: каскадный (выход каскада, его правит только
//! подготовка документа), вычисленный (материализуется из каскадного,
//! `style::cascade::materialize`) и слой used-значений раскладки поверх вычисленного. Раскладка
//! читает вычисленный стиль и пишет только в свой слой: каскадный и вычисленный стили остаются
//! общими для всех копий элемента (Blink: `LayoutObject::Style()` — неизменяемый вычисленный стиль;
//! Servo: `Arc<ComputedValues>`).

use crate::box_facts::BoxContext;
use crate::style::ComputedHandle;
use crate::style::box_input::BoxStyleInput;
use crate::style::cascade::materialize::ComputedSlot;
use crate::style::computed::writing_mode::WritingMode;
use crate::style::computed::{Align, Computed};
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
    box_context: Option<Rc<BoxContext<WritingMode, Align>>>,
    /// Оформление inline в существующем транспорте синтетических коробок.
    decoration: Option<Rc<crate::style::background_paint::DecorationInput>>,
}

impl From<Computed> for SharedStyle {
    fn from(style: Computed) -> Self {
        Self {
            cascaded: Rc::new(style),
            computed: ComputedSlot::default(),
            used: None,
            box_context: None,
            decoration: Default::default(),
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
            box_context: None,
            decoration: Default::default(),
        }
    }
}

impl SharedStyle {
    /// The cascaded style (css-cascade-5 §4.2): input of the computed-value stage.
    pub(crate) fn cascaded(&self) -> &Computed {
        &self.cascaded
    }

    /// `vertical-align` of the element's own box (CSS 2.1 §10.8.1; not
    /// inherited): its computed value when the cascade declares one (`inherit`
    /// included, as in the UA rule `td, th { vertical-align: inherit }`), else
    /// the initial `baseline` (`None`). The computed style also hands an inline
    /// ancestor's value down to the text runs inside it, which a box must not
    /// take as its own.
    pub(crate) fn own_vertical_align(&self) -> Option<crate::style::computed::Align> {
        let declared = self.cascaded.vertical_align.is_some()
            || self
                .cascaded
                .explicit_inherit
                .contains(&crate::style::cascade::inherited_value::Property::VerticalAlign);
        if declared {
            self.used().vertical_align
        } else {
            None
        }
    }

    /// Edit the cascaded style (cascade and document preparation, before
    /// materialization): drops computed/used values, retaining the element's
    /// completed container-size input across render-time style copies.
    pub(crate) fn cascaded_mut(&mut self) -> &mut Computed {
        self.computed.invalidate();
        self.used = None;
        Rc::make_mut(&mut self.cascaded)
    }

    /// The materialization slot of the cascaded style.
    pub(crate) fn slot(&self) -> &ComputedSlot {
        &self.computed
    }

    /// Неизменяемая computed-стадия после материализации. Синтетика без
    /// materialized slot возвращает None, а не каскад или used-копию.
    pub(crate) fn published(&self) -> Option<ComputedHandle> {
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
        BoxStyleInput::new(
            style,
            self.box_context.clone(),
            self.decoration
                .as_deref()
                .map(|d| d.inline)
                .unwrap_or_default(),
            self.decoration
                .as_deref()
                .map(|d| d.background_paint)
                .unwrap_or_default(),
            self.decoration.as_deref().and_then(|d| d.canvas.clone()),
        )
    }

    /// Только собственный фон; потомки наследуют computed CSS, а не этот вход.
    pub(crate) fn set_background_paint(
        &mut self,
        paint: crate::style::background_paint::BackgroundPaint,
    ) {
        let mut decoration = self.decoration.as_deref().cloned().unwrap_or_default();
        decoration.background_paint = paint;
        self.decoration = Some(Rc::new(decoration)).filter(|d| **d != Default::default());
    }

    /// Собственный источник холста, не computed CSS корня и не наследуемый фон.
    pub(crate) fn set_canvas_background(
        &mut self,
        canvas: Option<crate::style::canvas_background::CanvasBackgroundSource>,
    ) {
        let mut decoration = self.decoration.as_deref().cloned().unwrap_or_default();
        decoration.canvas = canvas;
        self.decoration = Some(Rc::new(decoration)).filter(|d| **d != Default::default());
    }

    /// Сохраняет факты существующей синтетической коробки до A07.
    pub(crate) fn set_box_context(&mut self, context: Rc<BoxContext<WritingMode, Align>>) {
        self.box_context = Some(context);
    }

    /// The style of an element that layout made from a box style (a copy styled
    /// for a fragment, a dissolved `display: contents` child): its values are
    /// computed already and become its only stage.
    ///
    /// TRANSITIONAL(cascade-7b-layout-made): layout_made() выдаёт computed box values за cascade при повторном входе через Element; удалить после сохранения исходного style handle и отдельной передачи BoxStyle.
    pub(crate) fn layout_made(input: impl Into<BoxStyleInput>) -> Self {
        let BoxStyleInput {
            style,
            context,
            inline_decoration,
            background_paint,
            canvas_background,
        } = input.into();
        Self {
            cascaded: style,
            computed: ComputedSlot::default(),
            used: None,
            box_context: context,
            decoration: Some(Rc::new(crate::style::background_paint::DecorationInput {
                inline: inline_decoration,
                background_paint,
                canvas: canvas_background,
            }))
            .filter(|d| **d != Default::default()),
        }
    }

    /// Layout's used-value layer, copy-on-write over the computed style.
    ///
    /// TRANSITIONAL(cascade-7b-layout-made): used_mut() меняет единственную стадию нематериализованного стиля; удалить fallback после разделения стадий у всех синтетических коробок.
    pub(crate) fn used_mut(&mut self) -> &mut Computed {
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
    pub(crate) fn used_rc(&self) -> Option<Rc<Computed>> {
        self.used.clone()
    }

    fn base(&self) -> Rc<Computed> {
        self.computed.any().unwrap_or_else(|| self.cascaded.clone())
    }
}

#[cfg(test)]
mod tests;
