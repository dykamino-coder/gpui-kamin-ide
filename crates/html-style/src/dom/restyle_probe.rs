//! Targeted restyling of source DOM elements without rebuilding boxes.
//! It uses the full-build document rules, :has marks and entry context.
//!
//! Rules are gathered once per document value (RestyleCache::probe) and
//! serve both DOM states of each mutation batch, before and after writing.
//! Shadow documents use the full walk: scope and slot assignment require
//! a document-wide prepass.

use super::input::Admitted;
use super::invalidation::{Changed, Features, features};
use super::outcome::DocumentFailure;
use super::shadow::Scope;
use super::tree_index::{HasMarks, TreeIndex};
use super::walk::{self, Chains, Probed};
use crate::style::css::Media;
use crate::style::select::has::HasArg;
use crate::values::DocumentValues;
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

mod relational;
mod rules;
mod subtree;
pub use subtree::finish_boxes;

/// Правила документа одной ревизии для проб его элементов.
pub struct RestyleProbe {
    doc: Rc<Scope>,
    has_args: Vec<HasArg>,
    media: Media,
    /// Носитель и тема, для которых собраны правила (`RestyleCache::probe`).
    asked: Media,
    extra_css: Box<str>,
    /// Лист документа или тема читают атрибуты вне селекторов (`attr()`).
    reads_attr: crate::dom::attr_reads::AttrReads,
}

/// Проба документа между правками его ревизии: правила и контексты входа в
/// детей пробованных родителей (`walk::Chains`). Держит её документ, а не его
/// значения: записи предков ссылаются на значения.
#[derive(Default)]
pub struct RestyleCache {
    probe: Option<Rc<RestyleProbe>>,
    chains: Chains,
    /// Признаки потомков по именам пакета (`invalidation::features`): Blink
    /// держит такой набор на каждое имя листа, здесь он строится при первой
    /// правке имени.
    sets: Vec<(Changed, Rc<Features>)>,
    /// The `:has()` arguments (by place in the probe's) that read a name
    /// the batch probed now changes: their marks are made again for each
    /// probe (`relational`).
    remark: Vec<usize>,
    /// Whole-document markings since `take_remarks`.
    remarks: u64,
    /// The mutated elements of the batch probed now whose parents' census a
    /// rule reads (`targets`, `Features::reads_siblings`).
    mutated: Vec<Handle>,
    /// The last probe of each element probed since the rules were built, by
    /// address (the handle keeps the address taken): the style an element
    /// has now. Blink keeps it on the element (`Element::GetComputedStyle`,
    /// `element.cc:5594` reads it as the old style of a recalc) and Stylo in
    /// its data (`ElementData::styles`, `data.rs:310`); here only edited
    /// elements have one. It holds while every style a batch changes is
    /// probed after the batch (`invalidation::targets` finds them all) and
    /// goes with the cache when a batch rebuilds the document.
    last: HashMap<usize, (Handle, Option<Probed>)>,
    /// The `:has()` marks of the source DOM, those of the build that gave
    /// the probe (`keep_has`) or of the first probe that needs them. A batch
    /// that changes a name any `:has()` argument reads (`has_reads`) marks
    /// them again for each of its probes (`remark`), and `invalidation::targets`
    /// takes the anchors such a name may reach; a batch that inserts or
    /// removes nodes or changes element states rebuilds; a batch that edits
    /// texts drops them (`forget_has`). Blink likewise re-marks only for the
    /// features of `:has()` arguments (`StyleEngine::InvalidateAncestorsOrSiblingsAffectedByHas`,
    /// `style_engine.cc:1676`);
    /// here the marks of the build hold until such a feature changes.
    has: Option<HasMarks>,
}

/// Наборов признаков больше этого числа — начинаются заново.
const SETS: usize = 16;

/// More kept probes than this start over.
const LAST: usize = 4096;

impl RestyleCache {
    /// Кэш, чья проба — правила только что собранного документа: первая
    /// правка не разбирает листы заново. Так Blink и Stylo держат индекс
    /// правил документа (`RuleSet`, `CascadeData`) между перерасчётами, а
    /// не строят его на мутацию.
    pub(super) fn with_probe(probe: RestyleProbe) -> Self {
        Self {
            probe: Some(Rc::new(probe)),
            ..Self::default()
        }
    }

    /// Проба документа значений `values`: правила собираются при первой
    /// правке и служат следующим, пока тема и носитель те же. Blink так же
    /// строит признаки правил один раз на лист (`RuleInvalidationData`,
    /// `rule_invalidation_data.cc:90`), а не на мутацию. Листы документа у
    /// одних значений не меняются: текст `<style>` прочитан сборкой
    /// (`TraversalState::read_source_text`), и его правка собирает документ
    /// заново с новыми значениями. `None` — документ с декларативными тенями.
    pub fn probe(
        &mut self,
        input: &Admitted,
        extra_css: &str,
        media: Media,
        values: &Rc<DocumentValues>,
    ) -> Result<Option<Rc<RestyleProbe>>, DocumentFailure> {
        let same = |p: &&Rc<RestyleProbe>| p.asked == media && *p.extra_css == *extra_css;
        if let Some(probe) = self.probe.as_ref().filter(same) {
            return Ok(Some(probe.clone()));
        }
        self.chains = Chains::default();
        self.sets.clear();
        self.last.clear();
        self.has = None;
        self.probe = RestyleProbe::new(input, extra_css, media, values)?.map(Rc::new);
        Ok(self.probe.clone())
    }

    /// The elements `mutated` and those whose style the batch of names
    /// `changed` may change (`invalidation::targets`): always a set, up to
    /// the whole document. `after`: the attributes of each mutated element
    /// after the batch.
    pub fn targets(
        &mut self,
        probe: &RestyleProbe,
        changed: &Changed,
        (mutated, after): (&[Handle], &[&[html5ever::Attribute]]),
    ) -> Vec<Handle> {
        // A name read inside a `:has()` argument changes its marks: each
        // probe of this batch (before and after it) marks them again.
        let reads = |a: &HasArg| super::invalidation::has_reads(std::slice::from_ref(a), changed);
        self.remark = (0..probe.has_args.len())
            .filter(|&i| reads(&probe.has_args[i]))
            .collect();
        let same =
            |(c, _): &&(Changed, _)| c.classes == changed.classes && c.attrs == changed.attrs;
        let features = match self.sets.iter().find(same) {
            Some((_, features)) => features.clone(),
            None => {
                let features = Rc::new(features(probe.rules(), changed));
                if self.sets.len() == SETS {
                    self.sets.clear();
                }
                let names = Changed {
                    elements: Vec::new(),
                    ..changed.clone()
                };
                self.sets.push((names, features.clone()));
                features
            }
        };
        // A parent's census of its children (`Chains`) answers sibling
        // compounds only: a batch no rule reads beside its elements keeps
        // every context, and each probe enters its targets alone instead of
        // cascading their ancestor chains again.
        self.mutated = match features.reads_siblings() {
            true => mutated.to_vec(),
            false => Vec::new(),
        };
        features.targets(mutated, after)
    }

    /// Вход в каждый элемент `targets` при текущем состоянии исходного DOM.
    pub fn styles(
        &mut self,
        probe: &RestyleProbe,
        input: &Admitted,
        values: &Rc<DocumentValues>,
        targets: &[Handle],
    ) -> Result<Vec<Option<Probed>>, DocumentFailure> {
        let mut tree = TreeIndex::new(values.clone(), input.xhtml);
        let document = &input.dom.document;
        self.chains.forget_parents(&self.mutated);
        if !probe.has_args.is_empty() {
            self.marks(document, &probe.has_args, &mut tree)?;
        }
        let vcx = values.values().with_scheme(scheme(probe.media));
        let chains = &mut self.chains;
        let found = walk::probe(document, &tree, &probe.doc, targets, chains, vcx)?;
        if !probe.has_args.is_empty() {
            self.has = Some(tree.take_has());
        }
        if self.last.len() + targets.len() > LAST {
            self.last.clear();
        }
        for (target, probed) in targets.iter().zip(&found) {
            let address = Rc::as_ptr(target) as usize;
            self.last.insert(address, (target.clone(), probed.clone()));
        }
        Ok(found)
    }

    /// Drop the kept `:has()` marks: a batch edited texts, which `:empty`
    /// and other arguments may read.
    pub fn forget_has(&mut self) {
        self.has = None;
    }

    /// Keep the `:has()` marks of the build that gave this cache its probe
    /// (`with_probe`): they mark the source DOM as the build found it, with
    /// the arguments of the probe, as a first probe would mark it again.
    /// Blink likewise keeps the `:has()` flags its first recalc set on the
    /// elements (`ComputedStyle::AffectedBySubjectHas`).
    pub(super) fn keep_has(&mut self, has: HasMarks) {
        self.has = Some(has);
    }

    /// The entries of `targets` at the current state of the source DOM, as
    /// `styles` gives them: the kept probes if every target has one, else a
    /// probe of them all. The list item number of an entry counts only the
    /// entries of its probe (`walk::probe`): a restyle compares whether the
    /// element is a list item, not the number.
    pub fn kept_styles(
        &mut self,
        probe: &RestyleProbe,
        input: &Admitted,
        values: &Rc<DocumentValues>,
        targets: &[Handle],
    ) -> Result<Vec<Option<Probed>>, DocumentFailure> {
        let kept = |t: &Handle| {
            self.last
                .get(&(Rc::as_ptr(t) as usize))
                .map(|(_, p)| p.clone())
        };
        match targets.iter().map(kept).collect::<Option<Vec<_>>>() {
            Some(kept) => Ok(kept),
            None => self.styles(probe, input, values, targets),
        }
    }
}

fn scheme(media: Media) -> crate::style::values::value::Scheme {
    crate::style::values::value::Scheme::new(media.dark)
}

#[cfg(test)]
mod tests;
