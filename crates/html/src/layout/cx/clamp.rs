//! Чтение плана и измерений активного clamp-контейнера из входов сборки.

use super::LayoutCx;

impl LayoutCx {
    pub(crate) fn clamp_measure(&self) -> Option<(u64, crate::text::clamp::ClampMeasure)> {
        match &self.clamp_input {
            Some(crate::text::clamp::ClampInput::Measure(measure))
            | Some(crate::text::clamp::ClampInput::ApplyMeasured(_, measure)) => {
                crate::text::clamp::clamp_context(self).map(|(key, _)| (key, measure.clone()))
            }
            _ => None,
        }
    }

    pub(crate) fn clamp_plan(&self, key: u64) -> Option<&crate::text::clamp::ClampPlan> {
        if let Some(input) = &self.clamp_input {
            if crate::text::clamp::clamp_context(self).is_none_or(|(own, _)| own != key) {
                return None;
            }
            return match input {
                crate::text::clamp::ClampInput::Apply(plan)
                | crate::text::clamp::ClampInput::ApplyMeasured(plan, _) => Some(plan),
                crate::text::clamp::ClampInput::Measure(_) => None,
            };
        }
        None
    }

    pub(crate) fn clamp_box_height(&self, node: u64) -> Option<f32> {
        match &self.clamp_input {
            Some(crate::text::clamp::ClampInput::Measure(_)) => None,
            Some(crate::text::clamp::ClampInput::Apply(plan))
            | Some(crate::text::clamp::ClampInput::ApplyMeasured(plan, _)) => plan
                .sizes
                .iter()
                .find(|(key, _)| *key == node)
                .map(|(_, h)| *h),
            None => None,
        }
    }

}
