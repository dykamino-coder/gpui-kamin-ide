//! Шаг документа до кадра (HTML «update the rendering»): сборка и раскладка
//! дерева кадра вне `Window::draw`.
//!
//! Шаг делает то же, что кадр делал бы сам: собирает дерево (`Builder`) и
//! раскладывает его корнем на точный размер области просмотра, как
//! `Retained::build_in`. Кадр берёт подготовленное дерево того же ключа и в
//! prepaint только проверяет заместителей и ставит дерево на место. Кадр,
//! для которого шаг ничего не подготовил (хост без шага, другие опции, кадр
//! пришлось собрать заново), собирается внутри `draw`, как раньше, и
//! считается в `builds_inside_draw`.

use super::{Builder, FrameKey, Slot};
use crate::Document;
use crate::layout::reuse::FlowReuse;
use gpui::{AnyElement, App, AvailableSpace, Window, px, size};
use std::cell::Cell;
use std::rc::Rc;

/// Дерево кадра, собранное и разложенное шагом документа.
pub(super) struct Prepared {
    pub(super) key: FrameKey,
    /// Сборка шага: повторная сборка в кадре продолжает её счётчики.
    pub(super) builder: Builder,
    pub(super) tree: AnyElement,
    pub(super) flow: Option<Rc<FlowReuse>>,
    /// Кадры, заказанные сборкой и раскладкой шага.
    pub(super) scheduled: usize,
}

/// Отметки шага для хоста, который меряет его фазы.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UpdatePhase {
    /// Дерево собрано, начинается запрос раскладки.
    Built,
    /// Раскладка разрешена; кадр получит готовое дерево.
    LaidOut,
}

impl Slot {
    /// Подготовленное дерево для кадра с ключом `key`; чужое отбрасывается.
    pub(super) fn take_prepared(&self, key: &Option<FrameKey>) -> Option<Prepared> {
        let prepared = self.prepared.borrow_mut().take()?;
        (key.as_ref() == Some(&prepared.key)).then_some(prepared)
    }
}

impl Document {
    /// Шаг документа перед кадром окна (`gpui::Window::on_update_rendering`):
    /// собрать и разложить кадр с опциями `opts`, если его не повторит
    /// записанный кадр. `observe` получает отметки фаз шага.
    pub fn update_rendering(
        &self,
        opts: &crate::RenderOpts,
        window: &mut Window,
        cx: &mut App,
        mut observe: impl FnMut(UpdatePhase),
    ) {
        self.retained.prepared.borrow_mut().take();
        let Some(key) = FrameKey::of(opts) else {
            return;
        };
        let candidate = *self.retained.recorded.borrow() == Some(key.clone())
            && self.state.values.resources_unchanged();
        if candidate {
            return;
        }
        let builder = self.frame_builder(opts, &Some(key.clone()));
        let (w, h) = builder.opts.viewport;
        let keys = builder.layout_keys;
        window.with_layout_outside_draw(cx, |window, cx| {
            let before = window.next_frame_callback_count();
            self.retained.builds.set(self.retained.builds.get() + 1);
            let (mut tree, flow) = builder.build(true, false);
            observe(UpdatePhase::Built);
            let space = size(
                AvailableSpace::Definite(px(w)),
                AvailableSpace::Definite(px(h)),
            );
            window.with_layout_keys(keys, |window| tree.layout_as_root(space, window, cx));
            observe(UpdatePhase::LaidOut);
            let scheduled = window.next_frame_callback_count().saturating_sub(before);
            *self.retained.prepared.borrow_mut() = Some(Prepared {
                key,
                builder,
                tree,
                flow,
                scheduled,
            });
        });
    }

    /// Кадры, собранные внутри `Window::draw`, а не шагом документа.
    pub fn builds_inside_draw(&self) -> u64 {
        self.retained.inside_draw.get()
    }

    pub(super) fn frame_builder(&self, opts: &crate::RenderOpts, key: &Option<FrameKey>) -> Builder {
        Builder {
            state: self.state.clone(),
            nodes: self.nodes.clone(),
            // The frame builds into this document: the host's empty one is never made.
            opts: opts.with_doc(self.state.clone()),
            store: self.reuse.clone(),
            counters: Cell::new(None),
            layout_keys: key.is_some() && self.state.values.resources_unchanged(),
        }
    }
}
