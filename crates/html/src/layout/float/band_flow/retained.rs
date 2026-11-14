//! Межкадровый план полос без элементов, LayoutId и рекордеров прошлого кадра.

use super::{BandFlow, Plan, Slot};
use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Default)]
pub(super) struct NumericPlans {
    window: Cell<Option<(f32, f32)>>,
    plans: RefCell<Vec<NumericPlan>>,
}

struct NumericPlan {
    width: f32,
    height: f32,
    slots: Vec<NumericSlot>,
}

/// Тип не может владеть Prepared, LayoutId, ClampMeasure или FlowGeometry.
struct NumericSlot {
    x: f32,
    y: f32,
    avail: f32,
    shapes: Option<super::Shapes>,
    float_margins: [f32; 2],
    h: Option<f32>,
    kids: Vec<NumericSlot>,
    b: f32,
    i: f32,
}

impl NumericSlot {
    fn take(slot: &Slot) -> Self {
        Self {
            x: slot.x,
            y: slot.y,
            avail: slot.avail,
            shapes: slot.shapes.clone(),
            float_margins: slot.float_margins,
            h: slot.h,
            kids: slot.kids.iter().map(Self::take).collect(),
            b: slot.b,
            i: slot.i,
        }
    }

    fn materialize(&self) -> Slot {
        Slot {
            x: self.x,
            y: self.y,
            avail: self.avail,
            shapes: self.shapes.clone(),
            float_margins: self.float_margins,
            h: self.h,
            kids: self.kids.iter().map(Self::materialize).collect(),
            b: self.b,
            i: self.i,
            ..Default::default()
        }
    }
}

impl NumericPlans {
    pub(super) fn get(&self, width: f32) -> Option<Rc<Plan>> {
        self.plans
            .borrow()
            .iter()
            .find(|p| p.width == width)
            .map(|p| {
                crate::work::add!(FloatNumericReuses, 1);
                Rc::new(Plan {
                    width: p.width,
                    height: p.height,
                    slots: p.slots.iter().map(NumericSlot::materialize).collect(),
                })
            })
    }

    pub(super) fn insert(&self, plan: &Plan) {
        let mut plans = self.plans.borrow_mut();
        if plans.len() == 4 {
            plans.remove(0);
        }
        plans.push(NumericPlan {
            width: plan.width,
            height: plan.height,
            slots: plan.slots.iter().map(NumericSlot::take).collect(),
        });
    }
}

impl BandFlow {
    pub(crate) fn retain_numeric(
        mut self,
        source: &Element,
        parent: &BoxStyle,
        opts: &RenderOpts,
    ) -> Self {
        self.plan = Rc::new(super::cache::Plans::retaining(
            opts.cx.layout().formatting.get(source, parent, opts),
        ));
        self
    }

    pub(super) fn validate_retained(&self, window: &gpui::Window) {
        if let Some(cache) = &self.plan.retained {
            let input = (f32::from(window.rem_size()), window.scale_factor());
            if cache.window.replace(Some(input)) != Some(input) {
                cache.plans.borrow_mut().clear();
                self.plan.clear();
            }
        }
    }
}
