//! Числовой layout input контейнера: копии коробок делят вход, но не used-правки.
use super::ComputedSlot;

impl ComputedSlot {
    pub fn container_input_id(&self) -> gpui::LayoutInputId {
        self.2
    }

    /// Правка каскада сбрасывает computed values, сохраняя вход раскладки элемента.
    pub fn invalidate(&mut self) {
        *self.0.get_mut() = None;
    }

    pub(super) fn compute(
        &self,
        parent: &crate::style::computed::Computed,
        own: &crate::style::computed::Computed,
        vcx: crate::values::ValueCx<'_>,
    ) -> crate::style::publication::Published {
        let mut computed = super::super::compute::build(parent, own, vcx);
        if super::sharing::size_key(computed.reset_context.cq_size)
            != super::sharing::size_key(self.1.get())
        {
            computed.reset_context.cq_size = self.1.get();
        }
        computed.finish()
    }

    /// Публикуется только результат завершённого layout через оркестратор.
    /// Изменение требует повторного прохода стиля до paint того же кадра.
    pub fn set_container_size(&self, size: (Option<f32>, Option<f32>)) -> bool {
        self.1.replace(size) != size
    }
}
