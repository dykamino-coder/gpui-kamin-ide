//! Сопоставление селекторов.
// owner: B

use crate::dom::tree_index::TreeIndex;
use crate::dom::{SlotInfo, content, local_name};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

pub mod has;
pub mod matching;

/// Цепочка предков для сопоставления `.card .title`: тег + классы + id.
#[derive(Clone)]
pub(crate) struct Ancestor {
    /// Computed counter directives follow DOM inheritance, even without a box.
    pub(crate) counter_style: [Option<String>; 3],
    pub(crate) counter_context: crate::style::generated::counter_style::Context,
    pub(crate) content_style: Option<Vec<crate::style::computed::ContentItem>>,
    pub(crate) content_none: Option<bool>,
    pub(crate) tag: String,
    /// Only HTML documents use ASCII case-insensitive names on HTML elements.
    pub(crate) html_attrs: bool,
    pub(crate) id: Option<String>,
    pub(crate) classes: Vec<String>,
    /// Все атрибуты узла: нужны атрибутным селекторам.
    pub(crate) attrs: Vec<(String, String)>,
    /// Место среди соседей: нужно структурным псевдоклассам.
    pub(crate) spot: Spot,
    /// Адрес ссылки: нужен `:link`/`:visited`.
    pub(crate) href: Option<String>,
    /// Атрибут `dir` самого узла (true = rtl): нужен `:dir()`.
    pub(crate) dir: Option<bool>,
    /// Отметки `:has()`: хеши аргументов, для которых узел — якорь с
    /// совпадением. Считаются отдельным проходом до обхода (см. `mark_has`).
    pub(crate) has_marks: Vec<u64>,
    /// Matches searching the host’s own shadow tree, outside :host(...).
    pub(crate) shadow_has_marks: Vec<u64>,
    /// Хост, увиденный ИЗНУТРИ своей тени: безликий (css-shadow-1 §3.1 —
    /// «the shadow host is featureless»), с ним совпадает только компаунд из
    /// `:host`/`:host()`. `Some` несёт цепочку предков хоста в его СВЕТЛОМ
    /// контексте — ею проверяется аргумент `:host(S)`.
    pub(crate) featureless: Option<Rc<Vec<Ancestor>>>,
    /// Слот дерева теней: его распределение (см. `TreeIndex::slots`).
    pub(crate) slot: Option<Rc<SlotInfo>>,
    /// Братья-элементы узла и его место среди них — у предка в цепочке
    /// `path`: нужны компаунду предка с соседним комбинатором (`div + div
    /// span`, Selectors-4 §16.3). У переписи братьев (`census_of`) пусто —
    /// там соседи приходят через `Sibs`.
    pub(crate) peers: Option<(Rc<Vec<Ancestor>>, usize)>,
    /// `scroll-target-group: auto` on this node: set once a link inside
    /// became its current scroll marker (css-overflow-5 §scroll-target-group).
    pub(crate) target_group: Option<Rc<std::cell::Cell<bool>>>,
    /// This link is the current scroll marker of its group (`:target-current`).
    pub(crate) target_current: bool,
    /// Нет детей-элементов и текста (`:empty`, selectors-4 §14.2).
    pub(crate) empty: bool,
    /// Элемент вне пространства имён XHTML (SVG, MathML): для `@namespace`
    /// по умолчанию (css-namespaces-3 §2.2).
    pub(crate) foreign: bool,
}

/// Режим quirks текущего документа (ставит его разбор; у рамки свой
/// документ — `document::state`).
pub(crate) fn quirks() -> bool {
    crate::document::state::current().quirks.get()
}

/// Место элемента среди соседей — по нему считаются структурные псевдоклассы.
#[derive(Clone, Copy, Default)]
pub(crate) struct Spot {
    /// Номер среди соседей-элементов, с единицы.
    pub(crate) index: usize,
    /// Сколько всего соседей-элементов.
    pub(crate) total: usize,
    /// То же, но среди соседей с ТЕМ ЖЕ тегом (`:nth-of-type`).
    pub(crate) of_type: usize,
    pub(crate) of_type_total: usize,
}

/// Братья узла: ВСЕ дети-элементы родителя и позиция узла среди них.
///
/// Соседним комбинаторам `+`/`~` хватает предыдущих, но
/// `:nth-last-child(… of S)` считает совпавших среди ПОСЛЕДУЮЩИХ
/// (селекторы-4 §child-index) — поэтому список полный.
#[derive(Clone, Copy)]
pub(crate) struct Sibs<'a> {
    pub(crate) all: &'a [Ancestor],
    /// Сколько элементов стоит ДО узла; сам узел-элемент = `all[pos]`.
    pub(crate) pos: usize,
    /// Узел — элемент и присутствует в `all[pos]`.
    pub(crate) is_elem: bool,
    /// Тот же список под `Rc`, если он есть: его забирает паспорт узла в
    /// цепочку предков (`Ancestor::peers`).
    pub(crate) rc: Option<&'a Rc<Vec<Ancestor>>>,
}

impl<'a> Sibs<'a> {
    pub(crate) const EMPTY: Sibs<'static> = Sibs {
        all: &[],
        pos: 0,
        is_elem: false,
        rc: None,
    };

    /// Предыдущие соседи-элементы — для `+` и `~`.
    pub(crate) fn prev(&self) -> &'a [Ancestor] {
        &self.all[..self.pos]
    }

    /// Последующие соседи-элементы.
    pub(crate) fn next(&self) -> &'a [Ancestor] {
        &self.all[self.pos + usize::from(self.is_elem)..]
    }

    /// Те же братья глазами элемента с номером `i` в общем списке.
    pub(crate) fn at(&self, i: usize) -> Sibs<'a> {
        Sibs {
            all: self.all,
            pos: i,
            is_elem: true,
            rc: self.rc,
        }
    }
}

/// Паспорт элемента для сопоставления селекторов.
pub(super) fn ancestor_of(child: &Handle, spot: Spot, tree: &TreeIndex) -> Option<Ancestor> {
    let NodeData::Element { name, attrs, .. } = &child.data else {
        return None;
    };
    let attrs = attrs.borrow();
    let find = |key: &str| {
        attrs
            .iter()
            .find(|a| &*a.name.local == key)
            .map(|a| a.value.to_string())
    };
    Some(Ancestor {
        counter_style: Default::default(),
        counter_context: Default::default(),
        content_style: None,
        content_none: None,
        tag: local_name(&name.local),
        html_attrs: content::html_attributes(&name.ns),
        id: find("id"),
        classes: find("class")
            .map(|v| v.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default(),
        attrs: attrs
            .iter()
            .map(|a| (a.name.local.to_string(), a.value.to_string()))
            .collect(),
        spot,
        href: find("href"),
        dir: find("dir").and_then(|v| match v.to_ascii_lowercase().as_str() {
            "rtl" => Some(true),
            "ltr" => Some(false),
            _ => None,
        }),
        has_marks: tree.has_marks_of(child),
        shadow_has_marks: vec![],
        featureless: None,
        slot: tree.slot_of(child),
        peers: None,
        target_group: None,
        target_current: false,
        empty: is_empty_node(child),
        foreign: &*name.ns != "http://www.w3.org/1999/xhtml",
    })
}

/// Перепись детей уровня: места и паспорта всех элементов.
pub(crate) fn census_of(children: &[Handle], tree: &TreeIndex) -> (Vec<Spot>, Vec<Ancestor>) {
    let tags: Vec<Option<String>> = children
        .iter()
        .map(|c| match &c.data {
            NodeData::Element { name, .. } => Some(name.local.to_string()),
            _ => None,
        })
        .collect();
    let total = tags.iter().filter(|t| t.is_some()).count();
    let mut seen = 0usize;
    let mut seen_of_type: HashMap<String, usize> = HashMap::new();
    let mut spots: Vec<Spot> = Vec::with_capacity(children.len());
    let mut all: Vec<Ancestor> = Vec::with_capacity(total);
    for (child, tag) in children.iter().zip(&tags) {
        let spot = match tag {
            Some(tag) => {
                seen += 1;
                let of_type = seen_of_type.entry(tag.clone()).or_insert(0);
                *of_type += 1;
                Spot {
                    index: seen,
                    total,
                    of_type: *of_type,
                    of_type_total: tags.iter().filter(|t| t.as_deref() == Some(tag)).count(),
                }
            }
            None => Spot::default(),
        };
        spots.push(spot);
        if let Some(a) = ancestor_of(child, spot, tree) {
            all.push(a);
        }
    }
    (spots, all)
}

/// `:empty` (selectors-4 §14.2): ни детей-элементов, ни текста; комментарии и
/// инструкции обработки не считаются, а пробельный текст — считается, как в
/// браузерах (`image-light-dark`: `<div></div>` пуст, `<div><div>` — нет).
pub(crate) fn is_empty_node(handle: &Handle) -> bool {
    handle.children.borrow().iter().all(|c| match &c.data {
        NodeData::Element { .. } => false,
        NodeData::Text { contents } => contents.borrow().is_empty(),
        _ => true,
    })
}
