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

mod box_repaint;
#[cfg(test)]
mod box_repaint_tests;
#[cfg(test)]
mod child_removal_tests;
#[cfg(test)]
mod column_repaint_tests;
#[cfg(test)]
mod column_unseen_tests;
#[cfg(test)]
mod content_replay_tests;
#[cfg(test)]
mod content_resize_tests;
mod counters;
mod element;
#[cfg(test)]
mod escaped_replay_tests;
#[cfg(test)]
mod far_tests;
#[cfg(test)]
mod float_root_replay_tests;
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
mod prepared;
#[cfg(test)]
mod rebuild_key_tests;
#[cfg(test)]
mod recolour_layout_tests;
pub use prepared::UpdatePhase;
#[cfg(test)]
mod band_rerecord_tests;
#[cfg(test)]
mod band_value_tests;
mod record;
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
mod window_key;

use super::Document;
use super::state::DocState;
use crate::dom::Node;
use crate::layout::reuse::FlowReuse;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use std::cell::Cell;
use std::rc::Rc;

/// Записи сохраняемого кадра одного дерева документа.
#[derive(Default)]
pub(super) struct Slot {
    /// Опции хоста последнего записанного устоявшегося кадра.
    recorded: std::cell::RefCell<std::rc::Weak<record::Inputs>>,
    /// Отложенные отрисовки дерева рисовали группу композиции после записи
    /// его кадра: такое дерево больше не повторяется.
    unreplayable: Cell<bool>,
    /// Сборки и повторы кадра — наблюдение для тестов и стенда.
    builds: Cell<u64>,
    replays: Cell<u64>,
    /// Кадр, собранный и разложенный шагом документа (`prepared`).
    prepared: std::cell::RefCell<Option<prepared::Prepared>>,
    /// Сборки внутри `Window::draw` — наблюдение для тестов и стенда.
    inside_draw: Cell<u64>,
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

/// Сборка кадра, которую элемент выполняет сам, если повтор не подошёл.
struct Builder {
    state: Rc<DocState>,
    nodes: Rc<Vec<Node>>,
    /// Номер дерева `nodes` (`Document::tree`).
    tree: u64,
    opts: crate::RenderOpts,
    store: Rc<crate::layout::reuse::Store>,
    /// Счётчики сборки до первой сборки кадра: повторная (после отказа
    /// заместителей) начинает с них, как первая.
    counters: Cell<Option<crate::layout::reuse::Counters>>,
    /// No resource changed since the previous frame: the window's frame tree
    /// may keep measured leaves by their layout keys (gpui::Window::with_layout_keys).
    layout_keys: bool,
}

impl Builder {
    /// Коробка области просмотра с деревом документа: начальный содержащий
    /// блок точного размера, белый холст UA и текст окна из опций хоста.
    /// Блоки корневого потока записываются для следующих кадров, а при
    /// `allow` подходящие из прошлых записей замещаются (`layout::reuse`).
    /// `drawn` — сборка внутри prepaint, кадр уже начат (`Store::draw`).
    fn build(&self, allow: bool, drawn: bool) -> (AnyElement, Option<Rc<FlowReuse>>) {
        let (w, h) = self.opts.viewport;
        let again = self.counters.get().is_some();
        match self.counters.get() {
            Some(counters) => counters.write(&self.opts.cx),
            None => self
                .counters
                .set(Some(crate::layout::reuse::Counters::read(&self.opts.cx))),
        }
        let flow = FlowReuse::new(self.store.clone(), &self.nodes, allow, drawn, again);
        let mut opts = self.opts.clone();
        opts.cx.contracts.reuse = flow.clone();
        let tree = div()
            .w(px(w))
            .h(px(h))
            .bg(rgb(0xffffff))
            .text_size(self.opts.text.font_size)
            .font_family(self.opts.text.font_family.clone())
            .children(super::scope::render(
                &self.state,
                (&self.nodes, self.tree),
                &opts,
            ))
            .into_any_element();
        (tree, flow)
    }
}

impl Document {
    /// Собрать кадр документа в коробке области просмотра (`opts.viewport`)
    /// либо повторить прошлый, если ни один его вход не изменился.
    pub fn render_viewport(&self, opts: &crate::RenderOpts) -> AnyElement {
        let key = FrameKey::of(opts);
        let candidate =
            self.retained.inputs(key.as_ref()).is_some() && self.state.values.resources_unchanged();
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
}
