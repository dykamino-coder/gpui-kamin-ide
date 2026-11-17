//! Публикация ревизии без перерасчёта стиля по повреждению пакета (`Plan`).
//!
//! Повреждение решает, что делает следующий кадр, как change hint у Gecko
//! (`RestyleManager.cpp:3699`: без подсказки кадр не работает вовсе) и restyle
//! damage у Servo (`layout_impl.rs:861`, `can_skip_reflow_request_entirely`):
//! - `Clean` — пакет не меняет ни одного значения, которое читает сборка кадра:
//!   атрибуты элементов без коробки (`html`, `display: none`) или только
//!   `class` и `data-*` (`neutral`), которые читают лишь селекторы и `attr()`
//!   каскада, уже доказавшие, что от них ничего не зависит. Дерево коробок —
//!   прежнее или с новыми атрибутами (тогда оно равно свежей сборке по
//!   значению), значения документа те же, поэтому ревизия сохраняет
//!   сохраняемый кадр и факты раскладки: следующий кадр — повтор записанного.
//! - `Patched` — копия путей к правленым коробкам (`patch`); у новой ревизии
//!   новый сохраняемый кадр, а факты прошлого дерева служат основой фактов
//!   нового (`DocState::adopting`): обходятся только новые списки детей.
//! - `Restyled` — копия путей к перестилизованным коробкам. Если изменились
//!   только значения, которые берёт слой сцены документа (`paint_damage`),
//!   они пишутся в слой (`layer_write`), а ревизия сохраняет кадр, как
//!   `Clean`; иначе — новый сохраняемый кадр, как `Patched`.

use crate::document::{Document, state::DocState};
use crate::dom::Node;
use std::rc::Rc;

/// Дерево коробок после пакета, который не меняет каскада.
pub(super) enum Plan {
    /// Кадр прежний: дерево без правленых коробок (`None`) или с новыми
    /// атрибутами, которых сборка кадра не читает.
    Clean(Option<Vec<Node>>),
    /// Дерево с копией только путей к правкам.
    Patched(Vec<Node>),
    /// Copied paths to restyled boxes (`repaint_plan`), with the node ids of
    /// the elements whose cascaded values changed: the block records are
    /// compared as at a rebuild (`layout::reuse::Store::restyle`). The paths
    /// to every box of those elements, in tree order, address them in both
    /// trees: the restyle does not change the tree's shape.
    Restyled(Vec<Node>, Vec<u64>, Vec<(super::patch::Path, u64)>),
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
        crate::paint_path::note_plan(match &plan {
            Plan::Clean(_) => &crate::paint_path::PLAN_CLEAN,
            Plan::Patched(_) => &crate::paint_path::PLAN_PATCHED,
            Plan::Restyled(..) => &crate::paint_path::PLAN_RESTYLED,
        });
        match plan {
            Plan::Clean(nodes) => {
                if let Some(nodes) = nodes {
                    self.nodes = Rc::new(nodes);
                    self.tree = crate::document::scope::next_tree();
                }
                self.keep_frame(
                    key,
                    revision,
                    #[cfg(feature = "engine-bench")]
                    timings,
                );
            }
            // Only values the scene layer takes changed: they are written
            // into it, and the recorded frame (`Slot`) replays with them.
            Plan::Restyled(nodes, restyled, paths) if self.write_paint_values(&nodes, &paths) => {
                self.reuse.restyle(&self.nodes, &nodes, &restyled);
                self.nodes = Rc::new(nodes);
                self.keep_frame(
                    key,
                    revision,
                    #[cfg(feature = "engine-bench")]
                    timings,
                );
            }
            Plan::Patched(nodes) => {
                *self = self.adopted(
                    nodes,
                    None,
                    key,
                    revision,
                    #[cfg(feature = "engine-bench")]
                    timings,
                );
            }
            Plan::Restyled(nodes, restyled, _) => {
                *self = self.adopted(
                    nodes,
                    Some(&restyled),
                    key,
                    revision,
                    #[cfg(feature = "engine-bench")]
                    timings,
                );
            }
        }
    }

    /// Ревизия `key` с прежним кадром, состоянием и фактами раскладки.
    fn keep_frame(
        &mut self,
        key: u64,
        revision: &crate::budget::RevisionBudget,
        #[cfg(feature = "engine-bench")] timings: crate::document::BuildTimings,
    ) {
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

    /// Write the paint values the restyle of the boxes at `paths` changed
    /// from the current tree to `nodes` into the scene layer of the last
    /// recorded frame; `false`, with nothing written, when there is no such
    /// layer or a change is not a layer write (`layer_write`).
    fn write_paint_values(&self, nodes: &[Node], paths: &[(super::patch::Path, u64)]) -> bool {
        let _timer = crate::paint_path::Timer::start();
        let (Some(layer), true) = (self.reuse.scene_layer(), self.frame_recorded()) else {
            // A frame replays only from a settled record of this tree.
            crate::paint_path::add(&crate::paint_path::REFUSED_NO_LAYER, 1);
            return false;
        };
        let own = |a: &crate::dom::Element, b: &crate::dom::Element| {
            super::layer_write::own_paint_of(&self.state, a, b)
        };
        // A box under a block the frame skipped: any block on its path.
        let unseen = |path: &super::patch::Path| {
            self.reuse.skipped_any()
                && (1..=path.len()).any(|k| {
                    super::patch::element_at(&self.nodes, &path[..k])
                        .is_some_and(|e| self.reuse.unpainted(e.node_id))
                })
        };
        // A box whose runs showed no pixel of the layer's frame.
        let off = |id: u64| self.reuse.off_screen(&layer, id);
        let Some(taken) = super::layer_write::write_paint_values(
            &self.nodes,
            nodes,
            paths,
            &layer,
            own,
            unseen,
            off,
        ) else {
            return false;
        };
        self.reuse.queue_repaints(taken.records);
        self.reuse.defer_repaints(taken.deferred);
        true
    }

    /// Следующая ревизия с деревом `nodes` и прежними значениями документа;
    /// `restyled` — элементы с новыми каскадными значениями (`Plan::Restyled`).
    fn adopted(
        &self,
        nodes: Vec<Node>,
        restyled: Option<&[u64]>,
        key: u64,
        revision: &crate::budget::RevisionBudget,
        #[cfg(feature = "engine-bench")] timings: crate::document::BuildTimings,
    ) -> Document {
        let state = DocState::adopting(&self.state);
        let base = state.values.images.borrow().base().cloned();
        state.values.admit_revision(revision.clone(), base.as_ref());
        // Документ отпускает выведенные группы стиля в Drop.
        state.values.hold_document();
        match restyled {
            // A restyle reaches the records as a rebuild's does.
            Some(ids) => self.reuse.restyle(&self.nodes, &nodes, ids),
            // The records hold for the patched tree only by the next build's
            // comparison (`layout::reuse::Store::patch`).
            None => self.reuse.patch(),
        }
        Document {
            state,
            nodes: Rc::new(nodes),
            tree: crate::document::scope::next_tree(),
            retained: Default::default(),
            reuse: self.reuse.clone(),
            restyle: self.restyle.clone(),
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
