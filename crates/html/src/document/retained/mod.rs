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

mod element;
#[cfg(test)]
mod tests;
mod window_key;

use super::Document;
use super::state::DocState;
use crate::dom::Node;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use std::cell::Cell;
use std::rc::Rc;

/// Записи сохраняемого кадра одного дерева документа.
#[derive(Default)]
pub(super) struct Slot {
    /// Опции хоста последнего записанного устоявшегося кадра.
    recorded: std::cell::RefCell<Option<FrameKey>>,
    /// Отложенные отрисовки дерева рисовали группу композиции после записи
    /// его кадра: такое дерево больше не повторяется.
    unreplayable: Cell<bool>,
    /// Сборки и повторы кадра — наблюдение для тестов и стенда.
    builds: Cell<u64>,
    replays: Cell<u64>,
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
    opts: crate::RenderOpts,
}

impl Builder {
    /// Коробка области просмотра с деревом документа: начальный содержащий
    /// блок точного размера, белый холст UA и текст окна из опций хоста.
    fn build(&self) -> AnyElement {
        let (w, h) = self.opts.viewport;
        div()
            .w(px(w))
            .h(px(h))
            .bg(rgb(0xffffff))
            .text_size(self.opts.text.font_size)
            .font_family(self.opts.text.font_family.clone())
            .children(super::scope::render(&self.state, &self.nodes, &self.opts))
            .into_any_element()
    }
}

impl Document {
    /// Собрать кадр документа в коробке области просмотра (`opts.viewport`)
    /// либо повторить прошлый, если ни один его вход не изменился.
    pub fn render_viewport(&self, opts: &crate::RenderOpts) -> AnyElement {
        let key = FrameKey::of(opts);
        let candidate = key.is_some()
            && *self.retained.recorded.borrow() == key
            && self.state.values.resources_unchanged();
        let builder = Builder {
            state: self.state.clone(),
            nodes: self.nodes.clone(),
            opts: opts.clone(),
        };
        element::Retained::new(self.retained.clone(), key, builder, candidate).into_any_element()
    }

    /// Сколько раз кадр собран и сколько раз повторён (`render_viewport`).
    pub fn retained_frames(&self) -> (u64, u64) {
        (self.retained.builds.get(), self.retained.replays.get())
    }
}
