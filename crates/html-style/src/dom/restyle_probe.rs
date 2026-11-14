//! Перерасчёт стиля нескольких элементов исходного DOM ревизии без сборки
//! дерева коробок (`walk::probe`): те же правила, что у полной сборки
//! (`document_rules`), те же отметки `:has()` и тот же вход в элемент.
//!
//! Правила собираются один раз на значения документа (`RestyleCache::probe`) и
//! служат обоим состояниям DOM каждого пакета мутаций: до записи и после неё. Документ с тенями не пробуется: области и
//! слоты считаются отдельным предпроходом по всему документу.

use super::input::Admitted;
use super::invalidation::{Changed, Features, features};
use super::outcome::DocumentFailure;
use super::shadow::Scope;
use super::tree_index::TreeIndex;
use super::walk::{self, Chains, Probed};
use crate::style::css::{Media, Rule};
use crate::style::select::has::{HasArg, mark_has};
use crate::values::DocumentValues;
use markup5ever_rcdom::Handle;
use std::rc::Rc;

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
}

/// Наборов признаков больше этого числа — начинаются заново.
const SETS: usize = 16;

impl RestyleCache {
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
            mark_has(document, &probe.has_args, &mut vec![], &mut tree)?;
        }
        let vcx = values.values().with_scheme(scheme(probe.media));
        let chains = &mut self.chains;
        Ok(walk::probe(
            document, &tree, &probe.doc, targets, chains, vcx,
        )?)
    }
}

impl RestyleProbe {
    fn new(
        input: &Admitted,
        extra_css: &str,
        asked: Media,
        values: &Rc<DocumentValues>,
    ) -> Result<Option<Self>, DocumentFailure> {
        let mut media = asked;
        let admission = crate::budget::admission::Admission::new(input.context().budget().clone());
        let document = &input.dom.document;
        if super::shadow::has_declarative_shadow(document, &admission)? {
            return Ok(None);
        }
        if let Some(dark) = super::page_scheme::preference(document, media.dark, &admission)? {
            media.dark = dark;
        }
        let vcx = values.values().with_scheme(scheme(media));
        let rules =
            super::document_rules::document_rules(input, extra_css, media, &admission, vcx)?;
        let sheets =
            super::style_sources::document_style_tags(document, &admission, &values.traversal)?;
        let reads_attr =
            sheets.iter().any(|css| css.contains("attr(")) || extra_css.contains("attr(");
        Ok(Some(Self {
            doc: rules.doc,
            has_args: rules.has_args,
            media,
            asked,
            extra_css: extra_css.into(),
            reads_attr,
        }))
    }

    /// Лист документа или тема читают атрибуты элементов вне селекторов.
    pub fn reads_attr(&self) -> bool {
        self.reads_attr
    }

    /// Правила области документа (агент, тема, листы страницы).
    pub fn rules(&self) -> &[Rule] {
        &self.doc.rules
    }
}

fn scheme(media: Media) -> crate::style::values::value::Scheme {
    crate::style::values::value::Scheme::new(media.dark)
}
