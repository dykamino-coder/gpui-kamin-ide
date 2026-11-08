//! Публикация ревизии без перерасчёта стиля по повреждению пакета (`Plan`).
//!
//! Повреждение решает, что делает следующий кадр, как change hint у Gecko
//! (`RestyleManager.cpp:3699`: без подсказки кадр не работает вовсе) и restyle
//! damage у Servo (`layout_impl.rs:861`, `can_skip_reflow_request_entirely`):
//! - `Clean` — ни одна коробка не несёт правленого значения (атрибуты элементов
//!   без коробки: `html`, `display: none`). Дерево коробок то же, значения
//!   документа те же, поэтому ревизия сохраняет дерево, сохраняемый кадр и
//!   факты раскладки: следующий кадр — повтор записанного.
//! - `Patched` — копия путей к правленым коробкам (`patch`); у новой ревизии
//!   новый сохраняемый кадр, а факты прошлого дерева служат основой фактов
//!   нового (`DocState::adopting`): обходятся только новые списки детей.

use crate::document::{Document, state::DocState};
use crate::dom::Node;
use std::rc::Rc;

/// Дерево коробок после пакета, который не меняет каскада.
pub(super) enum Plan {
    /// Дерево коробок прежнее по значению: ни одного пути к правке.
    Clean,
    /// Дерево с копией только путей к правкам.
    Patched(Vec<Node>),
}

impl Document {
    /// Опубликовать ревизию `key` по плану; пакет уже записан в исходный DOM,
    /// соответствие исходных узлов узлам коробок не меняется.
    pub(super) fn commit(
        &mut self,
        plan: Plan,
        key: u64,
        revision: &crate::budget::RevisionBudget,
        #[cfg(feature = "engine-bench")] timings: crate::document::BuildTimings,
    ) {
        match plan {
            Plan::Clean => {
                let base = self.state.values.images.borrow().base().cloned();
                self.state
                    .values
                    .admit_revision(revision.clone(), base.as_ref());
                self.key = key;
                self.last_failure = None;
                #[cfg(feature = "engine-bench")]
                {
                    self.timings = timings;
                }
            }
            Plan::Patched(nodes) => {
                *self = self.adopted(
                    nodes,
                    key,
                    revision,
                    #[cfg(feature = "engine-bench")]
                    timings,
                );
            }
        }
    }

    /// Следующая ревизия с деревом `nodes` и прежними значениями документа.
    fn adopted(
        &self,
        nodes: Vec<Node>,
        key: u64,
        revision: &crate::budget::RevisionBudget,
        #[cfg(feature = "engine-bench")] timings: crate::document::BuildTimings,
    ) -> Document {
        let state = DocState::adopting(&self.state);
        let base = state.values.images.borrow().base().cloned();
        state.values.admit_revision(revision.clone(), base.as_ref());
        Document {
            state,
            nodes: Rc::new(nodes),
            retained: Default::default(),
            reuse: self.reuse.clone(),
            root: self.root.clone(),
            key,
            theme_css: self.theme_css.clone(),
            base_url: self.base_url.clone(),
            xml: self.xml,
            last_failure: None,
            #[cfg(feature = "engine-bench")]
            timings,
        }
    }
}
