//! What the edits in place of one document keep between them (`Kept`).

use super::boxes::Boxes;
use super::mentions::Mentions;
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
    /// The names those sheets mention with the agent sheet and the theme of
    /// the index (kept_mentions).
    mentions: Option<(Box<str>, Rc<Mentions>)>,
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
            mentions: None,
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

    /// The names the agent sheet, the theme `theme_css` and the `<style>`
    /// sheets of this document mention (`Mentions`), indexed on the first
    /// edit in place: Blink builds its `RuleFeatureSet` once for a set of
    /// sheets (`rule_feature_set.cc`), not on each mutation, and the sheets
    /// of a document do not change in place. `sheets`: the texts of the
    /// `<style>` elements when already known (an update's markup), indexed
    /// anew unless they are the kept ones.
    pub(super) fn kept_mentions(
        &self,
        root: &markup5ever_rcdom::Handle,
        sheets: Option<&[String]>,
        theme_css: &str,
    ) -> Rc<Mentions> {
        let quirks = self.state.values.quirks();
        let index = |sheets: &[String]| {
            let mut all = vec![crate::style::ua::user_agent_css(), theme_css];
            if quirks {
                all.push(crate::style::ua::QUIRKS_CSS);
            }
            all.extend(sheets.iter().map(String::as_str));
            Rc::new(Mentions::of(all))
        };
        let mut kept = self.restyle.take().unwrap_or_default();
        let own = |s: &[String]| kept.sheets.as_deref().is_some_and(|k| k.as_slice() == s);
        let mentions = match sheets {
            Some(s) if !own(s) => index(s),
            _ => match kept.mentions.as_ref().filter(|(t, _)| **t == *theme_css) {
                Some((_, m)) => m.clone(),
                None => {
                    let collect = || Rc::new(super::shape::sheets(root));
                    let m = index(kept.sheets.get_or_insert_with(collect));
                    kept.mentions = Some((theme_css.into(), m.clone()));
                    m
                }
            },
        };
        self.restyle.set(Some(kept));
        mentions
    }
}

#[cfg(test)]
mod tests {
    use crate::document::Document;
    use std::rc::Rc;

    #[test]
    fn the_mentions_of_the_sheets_are_indexed_once_per_document_and_theme() {
        let doc = Document::builder("<style>.a{color:red}</style><p class=a>x</p>", "")
            .build()
            .unwrap();
        let root = doc.source_dom();
        let first = doc.kept_mentions(&root, None, "");
        assert!(Rc::ptr_eq(&first, &doc.kept_mentions(&root, None, "")));
        assert!(first.any(&[], &["a".into()]) && !first.any(&[], &["b".into()]));
        // Known sheets other than the kept ones, or another theme, index anew.
        let other = [String::from(".b{color:red}")];
        assert!(
            doc.kept_mentions(&root, Some(&other), "")
                .any(&[], &["b".into()])
        );
        let themed = doc.kept_mentions(&root, None, ".t{}");
        assert!(!Rc::ptr_eq(&first, &themed) && themed.any(&[], &["t".into()]));
    }
}
