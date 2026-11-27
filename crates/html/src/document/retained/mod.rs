//! Сохраняемый кадр документа: коробка области просмотра, которую неизменный
//! документ повторяет из прошлого кадра без сборки, раскладки, prepaint и
//! краски.
//!
//! Вход — опции хоста (`RenderOpts`), выход — один элемент размера области
//! просмотра. Кадр повторяется, только если все его входы прежние: то же
//! дерево (новое дерево — новый `Slot`), те же опции хоста, ресурсы без новой
//! ревизии, а в окне — те же границы, маска, стиль текста, масштаб, rem, время
//! анимаций и ни одного ввода или смены фокуса (`element::WindowKey`).
//! Записывается только устоявшийся кадр: документ сообщил готовность
//! (`frame_ready`), ни один элемент не заказал следующий кадр, а команды
//! краски переносимы в другой кадр (`gpui::RetainedPaint::replayable`), в
//! том числе краска отложенных отрисовок (`Window::retained_replayable`).
//! Иначе кадр собирается заново, как `Document::render`.

#[cfg(test)]
mod animation_tests;
mod box_repaint;
#[cfg(test)]
mod box_repaint_tests;
#[cfg(test)]
mod child_removal_tests;
#[cfg(test)]
mod clip_replay_tests;
#[cfg(test)]
mod column_repaint_tests;
#[cfg(test)]
mod column_unseen_tests;
#[cfg(test)]
mod content_image_tests;
#[cfg(test)]
mod content_replay_tests;
#[cfg(test)]
mod content_resize_tests;
mod counters;
#[cfg(test)]
mod hover_repaint_tests;
#[cfg(test)]
mod hover_replay_tests;
mod snapshot;
pub use snapshot::FrameCounters;
#[cfg(test)]
mod decorated_fill_tests;
#[cfg(test)]
mod effects_repaint_tests;
mod element;
#[cfg(test)]
mod escaped_replay_tests;
#[cfg(test)]
mod far_tests;
#[cfg(test)]
mod float_root_replay_tests;
#[cfg(test)]
mod framed_layer_tests;
#[cfg(test)]
mod invalidation_tests;
#[cfg(test)]
mod kept_paragraph_tests;
#[cfg(test)]
mod key_reuse_tests;
#[cfg(test)]
mod l0_memory_tests;
#[cfg(test)]
mod l0_resource_tests;
#[cfg(test)]
mod l0_tests;
#[cfg(test)]
mod layer_order_tests;
#[cfg(test)]
mod layered_replay_tests;
#[cfg(test)]
mod moved_rows_tests;
#[cfg(test)]
mod paint_damage_tests;
#[cfg(all(test, windows))]
mod paint_pixels_tests;
mod prepared;
#[cfg(test)]
mod rebuild_key_tests;
#[cfg(test)]
mod recolour_layout_tests;
#[cfg(test)]
mod record_release_tests;
#[cfg(test)]
mod replaced_max_width_tests;
#[cfg(test)]
mod slot_display_tests;
pub use prepared::UpdatePhase;
#[cfg(test)]
mod attr_scope_tests;
#[cfg(test)]
mod band_rerecord_tests;
#[cfg(test)]
mod band_value_tests;
mod record;
pub(crate) use record::frame_miss_code;
mod builder;
#[cfg(test)]
mod replay_tests;
#[cfg(test)]
mod resize_tests;
#[cfg(test)]
mod restyle_run_tests;
#[cfg(test)]
mod reuse_tests;
#[cfg(test)]
mod svg_paint_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod unread_color_tests;
#[cfg(test)]
mod unseen_tests;
#[cfg(test)]
mod used_style_tests;
#[cfg(test)]
mod why_tests;
mod window_key;
use builder::Builder;

use super::Document;
use super::state::DocState;
use gpui::{AnyElement, IntoElement};
use std::cell::Cell;

/// Записи сохраняемого кадра одного дерева документа.
#[derive(Default)]
pub(super) struct Slot {
    /// Опции хоста последнего записанного устоявшегося кадра.
    recorded: std::cell::RefCell<std::rc::Weak<record::Inputs>>,
    /// Сборки и повторы кадра — наблюдение для тестов и стенда.
    builds: Cell<u64>,
    replays: Cell<u64>,
    /// Кадр, собранный и разложенный шагом документа (`prepared`).
    prepared: std::cell::RefCell<Option<prepared::Prepared>>,
    /// Сборки внутри `Window::draw` — наблюдение для тестов и стенда.
    inside_draw: Cell<u64>,
    /// Why the frame record was built instead of replayed (`record::FrameMiss`).
    misses: Cell<[u64; record::MISSES]>,
    /// Why the last painted record was not published for replay.
    unsettled: Cell<Option<record::FrameMiss>>,
}

/// Опции хоста, от которых зависит кадр. Остальные поля `RenderOpts` хосту
/// закрыты; опции уже начатого кадра (`frame_opts`) не сохраняются.
#[derive(Clone, PartialEq)]
pub(super) struct FrameKey {
    viewport: (f32, f32),
    text: gpui::TextStyle,
    normal_line_height: f32,
    freeze_scrollbars: bool,
}

impl FrameKey {
    fn of(opts: &crate::RenderOpts) -> Option<Self> {
        opts.cx.contracts.document_root.is_none().then(|| Self {
            viewport: opts.viewport,
            text: opts.text.clone(),
            normal_line_height: opts.normal_line_height,
            freeze_scrollbars: opts.cx.deferred.freeze_scrollbars,
        })
    }
}

impl Document {
    /// Собрать кадр документа в коробке области просмотра (`opts.viewport`)
    /// либо повторить прошлый, если ни один его вход не изменился.
    pub fn render_viewport(&self, opts: &crate::RenderOpts) -> AnyElement {
        let key = FrameKey::of(opts);
        let candidate =
            self.retained.inputs(key.as_ref()).is_ok() && self.state.values.resources_unchanged();
        let prepared = self.retained.take_prepared(&key);
        let builder = self.frame_builder(opts, &key);
        element::Retained::new(self.retained.clone(), key, builder, candidate, prepared)
            .into_any_element()
    }

    /// Есть записанный устоявшийся кадр этого дерева: следующий кадр с теми же
    /// опциями хоста может его повторить.
    pub(super) fn frame_recorded(&self) -> bool {
        self.retained.recorded.borrow().strong_count() != 0
    }

    /// Why the last recorded frame of this revision was kept from replay (DevTools `why`).
    pub(in crate::document) fn unpublished_reason(&self) -> Option<&'static str> {
        self.retained.unsettled.get().map(record::FrameMiss::code)
    }
}

#[cfg(test)]
mod extent_settle_tests;
#[cfg(test)]
mod moved_glyph_tests;

#[cfg(test)]
mod deferred_group_tests;
#[cfg(test)]
mod lifted_tests;
#[cfg(test)]
mod outline_scroll_tests;
