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
        // Идентичность украшающей коробки — факт элемента, а не вычисленное значение
        // (css-text-decor-3 §2.1): её несёт каскадный стиль, поэтому равные computed
        // стили соседей остаются одним общим значением.
        if computed.decor_box != 0 {
            computed.decor_box = 0;
        }
        if super::sharing::size_key(computed.reset_context.cq_size)
            != super::sharing::size_key(self.1.get())
        {
            computed.reset_context.cq_size = self.1.get();
        }
        computed.finish()
    }

    /// Публикуется только результат завершённого layout через оркестратор.
    /// Изменение требует повторного прохода стиля до paint того же кадра.
    /// A change is noted in the DevTools `why` journal at the caller: it
    /// lays the window's roots out again in the same draw.
    #[track_caller]
    pub fn set_container_size(&self, size: (Option<f32>, Option<f32>)) -> bool {
        let old = self.1.replace(size);
        if old != size {
            crate::why::note("layout_retry.container_size", || {
                format!("container size (width, height) {old:?} -> {size:?}")
            });
        }
        old != size
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_changed_container_size_is_noted_at_the_caller() {
        let (_one, _sub) = crate::why::test_reader();
        let slot = super::ComputedSlot::default();
        let line = line!() + 1;
        assert!(slot.set_container_size((Some(800.0), None)));
        assert!(!slot.set_container_size((Some(800.0), None)));
        let j = crate::why::take();
        let mine: Vec<_> = j
            .causes
            .iter()
            .filter(|c| c.code == "layout_retry.container_size")
            .collect();
        assert_eq!(mine.len(), 1, "an unchanged size is no retry");
        assert!(
            mine[0]
                .site()
                .ends_with(&format!("container_input.rs:{line}")),
            "{}",
            mine[0].site()
        );
        assert!(
            mine[0]
                .detail
                .contains("(None, None) -> (Some(800.0), None)"),
            "{}",
            mine[0].detail
        );
    }
}
