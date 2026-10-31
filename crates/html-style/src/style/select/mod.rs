//! Сопоставление селекторов с узлами документа. Узел представлен паспортом
//! `Ancestor` (тег, id, классы, атрибуты, место среди соседей `Spot`, ссылка,
//! `dir`, отметки `:has()`, слот и тень), братья — `Sibs`; перепись детей
//! уровня (`census_of`) собирает их разом. Отметки `:has()` и режим quirks
//! принадлежат состоянию одного разбора документа.

use crate::dom::tree_index::TreeIndex;
use crate::dom::{SlotInfo, content, local_name};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

pub mod bloom;
pub mod cache;
pub mod has;
pub mod matching;
pub mod rule_index;

/// Цепочка предков для сопоставления `.card .title`: тег + классы + id.
#[derive(Clone)]
pub struct Ancestor {
    pub key: crate::dom::tree_index::NodeKey,
    pub matching: Rc<cache::MatchCaches>,
    pub ancestor_bloom: Option<bloom::Bloom>,
    /// Computed counter directives follow DOM inheritance, even without a box.
    pub counter_style: [Option<String>; 3],
    pub counter_context: crate::style::generated::counter_style::Context,
    pub content_style: Option<Vec<crate::style::computed::ContentItem>>,
    pub content_none: Option<bool>,
    pub tag: String,
    /// Only HTML documents use ASCII case-insensitive names on HTML elements.
    pub html_attrs: bool,
    pub id: Option<String>,
    pub classes: Vec<String>,
    /// Все атрибуты узла: нужны атрибутным селекторам.
    pub attrs: Vec<(String, String)>,
    /// Место среди соседей: нужно структурным псевдоклассам.
    pub spot: Spot,
    /// Адрес ссылки: нужен `:link`/`:visited`.
    pub href: Option<String>,
    /// Ссылка сопоставляется посещённой (`:visited`, иначе `:link`); каскад
    /// элемента снимает признак со «своей» ссылки по ограничению `:visited`
    /// (`style::css::visited`).
    pub visited: bool,
    /// Атрибут `dir` самого узла (true = rtl): нужен `:dir()`.
    pub dir: Option<bool>,
    /// Отметки `:has()`: хеши аргументов, для которых узел — якорь с
    /// совпадением. Считаются отдельным проходом до обхода (см. `mark_has`).
    pub has_marks: Vec<u64>,
    /// Matches searching the host’s own shadow tree, outside :host(...).
    pub shadow_has_marks: Vec<u64>,
    /// Хост, увиденный ИЗНУТРИ своей тени: безликий (css-shadow-1 §3.1 —
    /// «the shadow host is featureless»), с ним совпадает только компаунд из
    /// `:host`/`:host()`. `Some` несёт цепочку предков хоста в его СВЕТЛОМ
    /// контексте — ею проверяется аргумент `:host(S)`.
    pub featureless: Option<Rc<Vec<Ancestor>>>,
    /// Слот дерева теней: его распределение (см. `TreeIndex::slots`).
    pub slot: Option<Rc<SlotInfo>>,
    /// Братья-элементы узла и его место среди них — у предка в цепочке
    /// `path`: нужны компаунду предка с соседним комбинатором (`div + div
    /// span`, Selectors-4 §16.3). У переписи братьев (`census_of`) пусто —
    /// там соседи приходят через `Sibs`.
    pub peers: Option<(Rc<Vec<Ancestor>>, usize)>,
    /// `scroll-target-group: auto` on this node: set once a link inside
    /// became its current scroll marker (css-overflow-5 §scroll-target-group).
    pub target_group: Option<Rc<std::cell::Cell<bool>>>,
    /// This link is the current scroll marker of its group (`:target-current`).
    pub target_current: bool,
    /// Нет детей-элементов и текста (`:empty`, selectors-4 §14.2).
    pub empty: bool,
    /// Элемент вне пространства имён XHTML (SVG, MathML): для `@namespace`
    /// по умолчанию (css-namespaces-3 §2.2).
    pub foreign: bool,
    /// Язык содержимого (`lang`/`xml:lang` узла или унаследованный), который
    /// обход коробок передаёт вниз: пустой — язык неизвестен; `None` — паспорт
    /// собран вне обхода, язык ищется по цепочке предков.
    pub lang: Option<Rc<str>>,
}

/// Место элемента среди соседей — по нему считаются структурные псевдоклассы.
#[derive(Clone, Copy, Default)]
pub struct Spot {
    /// Номер среди соседей-элементов, с единицы.
    pub index: usize,
    /// Сколько всего соседей-элементов.
    pub total: usize,
    /// То же, но среди соседей с ТЕМ ЖЕ тегом (`:nth-of-type`).
    pub of_type: usize,
    pub of_type_total: usize,
}

/// Братья узла: ВСЕ дети-элементы родителя и позиция узла среди них.
///
/// Соседним комбинаторам `+`/`~` хватает предыдущих, но
/// `:nth-last-child(… of S)` считает совпавших среди ПОСЛЕДУЮЩИХ
/// (селекторы-4 §child-index) — поэтому список полный.
#[derive(Clone, Copy)]
pub struct Sibs<'a> {
    pub all: &'a [Ancestor],
    /// Сколько элементов стоит ДО узла; сам узел-элемент = `all[pos]`.
    pub pos: usize,
    /// Узел — элемент и присутствует в `all[pos]`.
    pub is_elem: bool,
    /// Тот же список под `Rc`, если он есть: его забирает паспорт узла в
    /// цепочку предков (`Ancestor::peers`).
    pub rc: Option<&'a Rc<Vec<Ancestor>>>,
}

impl<'a> Sibs<'a> {
    pub const EMPTY: Sibs<'static> = Sibs {
        all: &[],
        pos: 0,
        is_elem: false,
        rc: None,
    };

    /// Предыдущие соседи-элементы — для `+` и `~`.
    pub fn prev(&self) -> &'a [Ancestor] {
        &self.all[..self.pos]
    }

    /// Последующие соседи-элементы.
    pub fn next(&self) -> &'a [Ancestor] {
        &self.all[self.pos + usize::from(self.is_elem)..]
    }

    /// Те же братья глазами элемента с номером `i` в общем списке.
    pub fn at(&self, i: usize) -> Sibs<'a> {
        Sibs {
            all: self.all,
            pos: i,
            is_elem: true,
            rc: self.rc,
        }
    }
}

mod node;
pub use node::{ancestor_of, census_of, is_empty_node, link_visited};

#[cfg(test)]
mod tests;
