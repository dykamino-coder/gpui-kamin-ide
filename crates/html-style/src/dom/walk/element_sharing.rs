//! Разделение стиля между элементами (Stylo style sharing cache, Blink style
//! sharing): брат или кузен с тем же тегом, теми же атрибутами, равнозначной
//! цепочкой предков и тем же исходом перепроверяемых правил берёт готовый
//! результат каскада, нормализации и слоёв псевдостилей без сопоставления правил.
//!
//! Равнозначность цепочки задаёт номер разделения родителя: элемент, взявший
//! чужой стиль, получает номер владельца записи, прочие — новый. Равные номера
//! родителей значат, что на каждом уровне вверх совпали тег, атрибуты, отметки
//! `:has()` и исход перепроверяемых правил, а значит и фильтр Блума предков,
//! кандидаты индекса, исход устойчивых правил (`SharingFacts`) и подсказок,
//! читающих атрибуты предков. Входы, которые
//! цепочка не покрывает, — область правил, унаследованные переменные (одна
//! `Rc` у разделивших родителей), схема цвета и окружение — сравниваются явно.

use super::pseudo_styles::PseudoLayers;
use super::revalidation::{Check, outcome};
use crate::budget::admission::{Admission, Buffer};
use crate::dom::shadow::Scope;
use crate::style::cascade::sharing::Environment;
use crate::style::computed::Computed;
use crate::style::css::{DeclBlock, VarMap};
use crate::style::select::rule_index::positional_text;
use crate::style::select::{Ancestor, Sibs};
use crate::style::values::value::Scheme;
use crate::values::ValueCx;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

/// Записей в кэше: как у кэша каскада, чередование до этого числа разных
/// входов не выбивает запись до её повтора.
const CAPACITY: usize = 128;

/// Готовый стиль элемента до счётчиков и коробок псевдоэлементов.
#[derive(Clone)]
pub(super) struct Shared {
    pub candidates: Rc<Buffer<usize>>,
    pub style: Box<Computed>,
    pub vars: Rc<VarMap>,
    pub scheme: Scheme,
    pub important: Option<DeclBlock>,
    pub layers: PseudoLayers,
    /// Номер разделения: равный у элементов с равнозначным стилем.
    pub share: u64,
}

/// Ключ поиска: входы элемента, которые не покрывает номер родителя.
pub(super) struct Key<'a> {
    hash: u64,
    parent: u64,
    scope: &'a Rc<Scope>,
    vars: &'a Rc<VarMap>,
    scheme: Scheme,
    html_attrs: bool,
    tag: &'a str,
    attrs: &'a [(String, String)],
    /// Отметки `:has()` элемента: они входят в фильтр Блума потомков, а через
    /// него — в их кандидатов индекса; равные номера родителей должны значить
    /// равные отметки всей цепочки.
    marks: Vec<u64>,
}

struct Entry {
    hash: u64,
    parent: u64,
    scope: Rc<Scope>,
    vars: Rc<VarMap>,
    scheme: Scheme,
    html_attrs: bool,
    tag: String,
    attrs: Vec<(String, String)>,
    marks: Vec<u64>,
    environment: Environment,
    /// Перепроверяемые кандидаты: номер правила и исходы `matches` и
    /// `matches_ignoring_pseudo` у владельца записи.
    checks: Vec<Check>,
    shared: Shared,
    _lease: Option<crate::budget::LiveReservation>,
}

impl<'a> Key<'a> {
    /// `None` — элемент вне разделения: корень и границы теней (номер 0),
    /// чужое пространство имён, слоты, ссылки-маркеры прокрутки, хост тени
    /// или `::slotted`-цель (`excluded`), инлайн-стиль с позиционными функциями.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        parent: u64,
        scope: &'a Rc<Scope>,
        vars: &'a Rc<VarMap>,
        vcx: ValueCx<'_>,
        me: &Ancestor,
        tag: &'a str,
        attrs: &'a [(String, String)],
        excluded: bool,
    ) -> Option<Self> {
        if parent == 0
            || excluded
            || me.foreign
            || me.slot.is_some()
            || me.featureless.is_some()
            || me.href.as_deref().is_some_and(|h| h.starts_with('#'))
            || attrs
                .iter()
                .any(|(k, v)| k == "style" && positional_text(v))
        {
            return None;
        }
        let mut h = DefaultHasher::new();
        (parent, tag, attrs, &me.has_marks).hash(&mut h);
        Some(Self {
            hash: h.finish(),
            parent,
            scope,
            vars,
            scheme: vcx.scheme,
            html_attrs: me.html_attrs,
            tag,
            attrs,
            marks: me.has_marks.clone(),
        })
    }
}

/// Кэш одного обхода документа.
#[derive(Default)]
pub(super) struct ElementSharing {
    entries: RefCell<VecDeque<Entry>>,
    next: Cell<u64>,
    /// Исходы перепроверки текущего элемента: `lookup` начинает их заново,
    /// `insert` после промаха того же элемента читает готовые.
    memo: RefCell<Vec<Check>>,
}

impl ElementSharing {
    /// Новый номер разделения (ноль зарезервирован за «без разделения»).
    pub fn fresh(&self) -> u64 {
        let next = self.next.get() + 1;
        self.next.set(next);
        next
    }

    pub fn lookup(
        &self,
        key: &Key,
        me: &Ancestor,
        path: &[Ancestor],
        sibs: Sibs,
        vcx: ValueCx<'_>,
    ) -> Option<Shared> {
        let entries = self.entries.borrow();
        let mut memo = self.memo.borrow_mut();
        memo.clear();
        let hit = entries.iter().rev().find(|e| {
            e.hash == key.hash
                && e.parent == key.parent
                && Rc::ptr_eq(&e.scope, key.scope)
                && Rc::ptr_eq(&e.vars, key.vars)
                && e.scheme == key.scheme
                && e.html_attrs == key.html_attrs
                && e.tag == key.tag
                && e.attrs == key.attrs
                && e.marks == key.marks
                && e.environment.matches(vcx)
                && e.checks
                    .iter()
                    .all(|&check| outcome(&mut memo, key.scope, check.0, me, path, sibs) == check)
        })?;
        crate::work::add!(ElementSharingHits, 1);
        Some(hit.shared.clone())
    }

    /// Запомнить результат промаха. Правила или переменные с позиционными
    /// значениями делают результат функцией места элемента — такой не
    /// запоминается; как и результат, записавший наблюдение в журнал документа
    /// (вызывающий сверяет diagnostic_observations): взявший его элемент
    /// записал бы его ещё раз.
    pub fn insert(
        &self,
        key: &Key,
        me: &Ancestor,
        path: &[Ancestor],
        sibs: Sibs,
        shared: &Shared,
        vcx: ValueCx<'_>,
    ) {
        let facts = &key.scope.index.sharing;
        if shared.candidates.iter().any(|&i| facts[i].positional)
            || key.vars.values().any(|v| positional_text(v))
        {
            return;
        }
        let mut memo = self.memo.borrow_mut();
        let checks: Vec<Check> = shared
            .candidates
            .iter()
            .filter(|&&i| facts[i].revalidate)
            .map(|&i| outcome(&mut memo, key.scope, i, me, path, sibs))
            .collect();
        let bytes = size_of::<Entry>()
            + key.tag.len()
            + checks.len() * size_of::<Check>()
            + key.marks.len() * size_of::<u64>()
            + key
                .attrs
                .iter()
                .map(|(k, v)| size_of::<(String, String)>() + k.len() + v.len())
                .sum::<usize>();
        let admission = Admission::new(vcx.doc.revision_budget());
        let Ok(lease) = admission.storage(bytes) else {
            return;
        };
        // Запись только перемещается в очередь: стиль владельца не меняется.
        let mut entries = self.entries.take();
        if entries.len() == CAPACITY {
            entries.pop_front();
        }
        entries.push_back(Entry {
            hash: key.hash,
            parent: key.parent,
            scope: key.scope.clone(),
            vars: key.vars.clone(),
            scheme: key.scheme,
            html_attrs: key.html_attrs,
            tag: key.tag.to_string().into(),
            attrs: key.attrs.to_vec().into(),
            marks: key.marks.clone(),
            environment: Environment::new(vcx),
            checks,
            shared: shared.clone(),
            _lease: lease,
        });
        self.entries.replace(entries);
    }
}

#[cfg(test)]
#[path = "element_sharing_tests.rs"]
mod tests;
