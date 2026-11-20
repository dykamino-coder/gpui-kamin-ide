//! The document rendering step resolves block reuse with the laid-out root.
//! The element consumes that decision when it draws in the same environment.

mod layout;
use super::{Builder, FrameKey, Slot};
use crate::Document;
use crate::layout::reuse::FlowReuse;
use gpui::{AnyElement, App, ElementPlace, Window, px, size};
pub(super) use layout::CheckedRoot;
use std::cell::Cell;
use std::rc::Rc;

/// Дерево кадра, собранное и разложенное шагом документа.
pub(super) struct Prepared {
    pub(super) key: FrameKey,
    /// Счётчики до сборки шага: повторная сборка в кадре начинает с них, как
    /// шаг (сборщик кадр делает свой — опции того же ключа).
    pub(super) counters: Option<crate::layout::reuse::Counters>,
    pub(super) tree: AnyElement,
    pub(super) flow: Option<Rc<FlowReuse>>,
    pub(super) checked: CheckedRoot,
    pub(super) layout_inputs: gpui::RetainedLayoutInputs,
    /// Кадры, заказанные сборкой и раскладкой шага.
    pub(super) scheduled: usize,
    /// Место в кадре, под которым стоит элемент документа этого дерева.
    pub(super) parent: ElementPlace,
}

/// Id элемента документа в кадре: его записи блоков. Они переживают правки
/// (новый `Slot`), и состояние элемента переживает их тоже.
pub(super) fn document_id(store: &Rc<crate::layout::reuse::Store>) -> gpui::ElementId {
    gpui::ElementId::NamedInteger(
        "retained-document".into(),
        Rc::as_ptr(store) as usize as u64,
    )
}

/// Отметки шага для хоста, который меряет его фазы.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UpdatePhase {
    /// Дерево собрано, начинается запрос раскладки.
    Built,
    /// Узлы раскладки запрошены, начинается её вычисление.
    Requested,
    /// Раскладка разрешена; кадр получит готовое дерево.
    LaidOut,
}

impl Slot {
    /// Подготовленное дерево для кадра с ключом `key`; чужое отбрасывается.
    pub(super) fn take_prepared(&self, key: &Option<FrameKey>) -> Option<Prepared> {
        let prepared = self.prepared.take()?;
        (key.as_ref() == Some(&prepared.key)).then_some(prepared)
    }
}

impl Document {
    /// Шаг документа перед кадром окна (`gpui::Window::on_update_rendering`):
    /// собрать и разложить кадр с опциями `opts`, если его не повторит
    /// записанный кадр. `parent` — место в кадре, где хост рисует
    /// `render_viewport` (корень view — `ElementPlace::of_view`); кадр в
    /// другом месте соберётся сам, как без шага. `observe` получает отметки
    /// фаз шага.
    pub fn update_rendering(
        &self,
        opts: &crate::RenderOpts,
        parent: &ElementPlace,
        window: &mut Window,
        cx: &mut App,
        mut observe: impl FnMut(UpdatePhase),
    ) {
        self.retained.prepared.take();
        let Some(key) = FrameKey::of(opts) else {
            return;
        };
        let (w, h) = key.viewport;
        let bounds = gpui::Bounds::new(Default::default(), size(px(w), px(h)));
        window.with_layout_outside_draw(parent, cx, |window, cx| {
            let id = document_id(&self.reuse);
            window.with_global_id(id, |id, window| {
                let environment =
                    super::window_key::WindowKey::capture(bounds, window, cx, &self.state);
                let (epoch, replay) =
                    window.with_element_state::<super::record::Record, _>(id, |record, window| {
                        let mut record = record.unwrap_or_default();
                        let epoch = record.begin(&self.reuse, window);
                        let replay = self.state.values.resources_unchanged()
                            && record
                                .replay(
                                    &self.retained,
                                    Some(&key),
                                    &environment,
                                    window,
                                    &self.reuse,
                                )
                                .is_some();
                        ((epoch, replay), record)
                    });
                if replay {
                    return;
                }
                let builder = self.frame_builder(opts, &Some(key.clone()));
                let before = window.next_frame_callback_count();
                let (tree, flow, layout_inputs) =
                    builder.layout_checked(bounds, window, cx, |phase| {
                        if phase == UpdatePhase::Built {
                            self.retained.builds.set(self.retained.builds.get() + 1);
                        }
                        observe(phase);
                    });
                let checked = CheckedRoot::new(epoch, environment, window);
                let scheduled = window.next_frame_callback_count().saturating_sub(before);
                self.retained.prepared.replace(Some(Prepared {
                    key,
                    counters: builder.counters.get(),
                    tree,
                    flow,
                    checked,
                    layout_inputs,
                    scheduled,
                    parent: parent.clone(),
                }));
            })
        });
    }

    /// Кадры, собранные внутри `Window::draw`, а не шагом документа.
    pub fn builds_inside_draw(&self) -> u64 {
        self.retained.inside_draw.get()
    }

    pub(super) fn frame_builder(
        &self,
        opts: &crate::RenderOpts,
        key: &Option<FrameKey>,
    ) -> Builder {
        Builder {
            state: self.state.clone(),
            nodes: self.nodes.clone(),
            tree: self.tree,
            // The frame builds into this document: the host's empty one is never made.
            opts: opts.with_doc(self.state.clone()),
            store: self.reuse.clone(),
            counters: Cell::new(None),
            layout_keys: key.is_some() && self.state.values.resources_unchanged(),
        }
    }
}
