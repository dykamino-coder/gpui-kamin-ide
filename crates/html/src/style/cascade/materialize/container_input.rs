//! Числовой layout input контейнера: копии коробок делят вход, но не used-правки.
use super::ComputedSlot;

impl ComputedSlot {
    pub(crate) fn container_input_id(&self) -> gpui::LayoutInputId {
        self.2
    }

    /// Правка каскада сбрасывает computed values, сохраняя вход раскладки элемента.
    pub(crate) fn invalidate(&mut self) {
        *self.0.get_mut() = None;
    }

    pub(super) fn compute(
        &self,
        parent: &crate::style::computed::Computed,
        own: &crate::style::computed::Computed,
        vcx: crate::document::state::ValueCx<'_>,
    ) -> crate::style::publication::Published {
        let mut computed = super::super::compute::build(parent, own, vcx);
        computed.cq_size = self.1.get();
        computed.finish()
    }

    /// Публикуется только результат завершённого layout через оркестратор.
    /// Изменение требует повторного прохода стиля до paint того же кадра.
    pub(crate) fn set_container_size(&self, size: (Option<f32>, Option<f32>)) -> bool {
        self.1.replace(size) != size
    }
}
