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
use crate::style::select::has::{HasArg, mark_has};
use crate::values::DocumentValues;
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

mod rules;

/// Правила документа одной ревизии для проб его элементов.
pub struct RestyleProbe {
    doc: Rc<Scope>,
    has_args: Vec<HasArg>,
    media: Media,
    /// Носитель и тема, для которых собраны правила (`RestyleCache::probe`).
    asked: Media,
    extra_css: Box<str>,
    /// Лист документа или тема читают атрибуты вне селекторов (`attr()`).
    reads_attr: bool,
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
    sets: Vec<(Changed, Option<Rc<Features>>)>,
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
    /// the probe (`keep_has`) or of the first probe that needs them. A batch probed here changes no value a `:has()` argument
    /// reads: `invalidation::targets` does not follow a name inside a
    /// relational or complex selector argument, so such a batch rebuilds, and `targets` drops the
    /// marks for a name any `:has()` argument reads (`has_reads`); a batch
    /// that inserts or removes nodes or changes element states rebuilds; a
    /// batch that edits texts
    /// drops them (`forget_has`). Blink likewise re-marks only for the
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

    /// Элементы `mutated` и те под ними, чей стиль может изменить пакет имён
    /// `changed` (`invalidation::targets`); `None` — любой элемент.
    pub fn targets(
        &mut self,
        probe: &RestyleProbe,
        changed: &Changed,
        mutated: &[Handle],
    ) -> Option<Vec<Handle>> {
        // A name read inside a `:has()` argument changes its marks. Such a
        // batch rebuilds anyway (`features` does not follow relational selector
        // arguments); the marks go first so no probe reads them stale.
        if super::invalidation::has_reads(&probe.has_args, changed) {
            self.has = None;
        }
        let same =
            |(c, _): &&(Changed, _)| c.classes == changed.classes && c.attrs == changed.attrs;
        let features = match self.sets.iter().find(same) {
            Some((_, features)) => features.clone(),
            None => {
                let features = features(probe.rules(), changed).map(Rc::new);
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
        features?.targets(mutated)
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
        if !probe.has_args.is_empty() {
            match self.has.take() {
                Some(has) => tree.put_has(has),
                None => mark_has(document, &probe.has_args, &mut vec![], &mut tree)?,
            }
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
    /// probe of them all. A list item probes again: the entries of one probe
    /// count list items in the order of its targets (`walk::probe`), so its
    /// number compares only between probes of the same targets.
    pub fn kept_styles(
        &mut self,
        probe: &RestyleProbe,
        input: &Admitted,
        values: &Rc<DocumentValues>,
        targets: &[Handle],
    ) -> Result<Vec<Option<Probed>>, DocumentFailure> {
        let kept = |t: &Handle| match self.last.get(&(Rc::as_ptr(t) as usize)) {
            Some((_, Some(p))) if p.rest.list_item.is_some() => None,
            Some((_, p)) => Some(p.clone()),
            None => None,
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
