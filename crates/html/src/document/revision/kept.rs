//! What the edits in place of one document keep between them (`Kept`).

use super::boxes::Boxes;
use super::source_index::SourceIndex;
use crate::document::Document;
use crate::dom::Node;
use std::rc::Rc;

/// What the edits of one document keep between them: the texts of its sheets,
/// the probe of its restyles, the index of its boxes and the numbers of its
/// source elements. The sheets of a
/// document do not change in place (a build reads the text of a `<style>`,
/// and an edit of a read text rebuilds), an edit in place keeps the shape of
/// the box tree (`commit::Plan`), and a rebuilt document starts over.
#[derive(Default)]
pub(in crate::document) struct Kept {
    sheets: Option<Rc<Vec<String>>>,
    pub(super) cache: crate::dom::restyle_probe::RestyleCache,
    boxes: Option<Rc<Boxes>>,
    ids: Option<Rc<SourceIndex>>,
}

impl Kept {
    /// What a built document keeps: the rules of its build as the probe.
    pub(in crate::document) fn built(
        cache: crate::dom::restyle_probe::RestyleCache,
    ) -> std::rc::Rc<std::cell::Cell<Option<Box<Self>>>> {
        let kept = Self {
            sheets: None,
            cache,
            boxes: None,
            ids: None,
        };
        Rc::new(std::cell::Cell::new(Some(Box::new(kept))))
    }

    /// The index of the boxes of `nodes`, built on the first edit in place.
    pub(super) fn boxes(&mut self, nodes: &[Node]) -> Option<Rc<Boxes>> {
        if self.boxes.is_none() {
            self.boxes = Some(Rc::new(Boxes::of(nodes)?));
        }
        self.boxes.clone()
    }

    /// An edit rebuilt some boxes (`repaint::rebox`): the indexes of the
    /// tree's shape and of its elements start over.
    pub(super) fn reshaped(&mut self) {
        self.boxes = None;
        self.ids = None;
    }

    /// The box numbers of the source elements `list` of the build, indexed on
    /// the first edit in place.
    pub(super) fn ids(&mut self, list: &[usize]) -> Option<Rc<SourceIndex>> {
        if self.ids.is_none() {
            self.ids = Some(Rc::new(SourceIndex::of(list)?));
        }
        self.ids.clone()
    }
}

impl Document {
    /// The texts of the `<style>` sheets under `root`, collected once.
    pub(super) fn kept_sheets(&self, root: &markup5ever_rcdom::Handle) -> Rc<Vec<String>> {
        let mut kept = self.restyle.take().unwrap_or_default();
        let collect = || Rc::new(super::shape::sheets(root));
        let sheets = kept.sheets.get_or_insert_with(collect).clone();
        self.restyle.set(Some(kept));
        sheets
    }

    /// The index of the boxes of this tree, built on the first edit in place.
    pub(super) fn boxes(&self) -> Option<Rc<Boxes>> {
        let mut kept = self.restyle.take().unwrap_or_default();
        let boxes = kept.boxes(&self.nodes);
        self.restyle.set(Some(kept));
        boxes
    }

    /// Drop the kept `:has()` marks of the probe (`RestyleCache::forget_has`).
    pub(super) fn forget_has(&self) {
        if let Some(mut kept) = self.restyle.take() {
            kept.cache.forget_has();
            self.restyle.set(Some(kept));
        }
    }

    /// The box numbers of the source elements of this build (`SourceIndex`).
    pub(super) fn source_index(&self) -> Option<Rc<SourceIndex>> {
        let mut kept = self.restyle.take().unwrap_or_default();
        let ids = kept.ids(&self.state.values.traversal.elements());
        self.restyle.set(Some(kept));
        ids
    }
}
