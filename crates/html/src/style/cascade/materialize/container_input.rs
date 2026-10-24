//! Числовой layout input контейнера: копии коробок делят вход, но не used-правки.
use super::ComputedSlot;

impl ComputedSlot {
    /// Правка каскада сбрасывает computed values, сохраняя вход раскладки элемента.
    pub(crate) fn invalidate(&mut self) {
        *self.0.get_mut() = None;
    }

    pub(super) fn compute(
        &self,
        parent: &crate::style::computed::Computed,
        own: &crate::style::computed::Computed,
        vcx: crate::document::state::ValueCx<'_>,
    ) -> std::rc::Rc<crate::style::computed::Computed> {
        let mut computed = super::super::compute::compute(parent, own, vcx);
        computed.cq_size = self.1.get();
        std::rc::Rc::new(computed)
    }

    /// Публикуется только результат завершённого layout через оркестратор.
    /// Изменение требует повторного прохода стиля до paint того же кадра.
    pub(crate) fn set_container_size(&self, size: (Option<f32>, Option<f32>)) -> bool {
        self.1.replace(size) != size
    }
}
