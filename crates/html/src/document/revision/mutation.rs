//! Записи мутаций DOM: единственный вход правки ревизии.
//!
//! Запись адресует узел исходного DOM ревизии (`Document::source_dom`) так же,
//! как ручка узла в JS, и несёт новое значение: `data` текстового узла (DOM
//! §4.10, сеттер `CharacterData.data`) или атрибут элемента (`setAttribute`,
//! `removeAttribute`). Так приходит правка в Blink (`CharacterData::
//! SetDataAndUpdate`, `character_data.cc:193`) и Servo (`CharacterData::
//! SetData`, `characterdata.rs:116`): без разбора и без сравнения деревьев.
//! `Document::update` сводит новую разметку той же формы к тем же записям
//! (`shape`), поэтому путь применения один.
//!
//! Пакет записей сводится к узлам (`Batch`): последнее значение каждого текста
//! и итоговый список атрибутов каждого элемента рядом с прежними. Прежние
//! значения нужны для отката: пакет, сборку которого отверг бюджет, не
//! оставляет следов в DOM прежней ревизии.

use html5ever::{Attribute, QualName};
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

/// Запись мутации исходного DOM ревизии.
#[derive(Clone)]
pub enum Mutation {
    /// Новое содержимое текстового узла `node`.
    CharacterData { node: Handle, data: String },
    /// Атрибут `name` элемента `element`: новое значение или `None` — снять.
    Attribute {
        element: Handle,
        name: QualName,
        value: Option<String>,
    },
}

/// Правка одного текста пакета: прежнее и новое содержимое.
pub(super) struct TextChange {
    pub(super) node: Handle,
    pub(super) old: String,
    pub(super) new: String,
}

/// Правка атрибутов одного элемента пакета: прежний и итоговый списки.
pub(super) struct AttrChange {
    pub(super) element: Handle,
    pub(super) old: Vec<Attribute>,
    pub(super) new: Vec<Attribute>,
}

/// Записи пакета, сведённые к узлам; правки без итогового отличия выброшены.
pub(super) struct Batch {
    pub(super) texts: Vec<TextChange>,
    pub(super) elements: Vec<AttrChange>,
}

impl Batch {
    /// `None` — запись адресует узел не того вида (текст вместо элемента).
    pub(super) fn new(records: &[Mutation]) -> Option<Self> {
        let mut batch = Self {
            texts: Vec::new(),
            elements: Vec::new(),
        };
        for record in records {
            match record {
                Mutation::CharacterData { node, data } => {
                    let NodeData::Text { contents } = &node.data else {
                        return None;
                    };
                    let at = match batch.texts.iter().position(|t| Rc::ptr_eq(&t.node, node)) {
                        Some(at) => at,
                        None => {
                            let old = contents.borrow().to_string();
                            batch.texts.push(TextChange {
                                node: node.clone(),
                                new: old.clone(),
                                old,
                            });
                            batch.texts.len() - 1
                        }
                    };
                    data.clone_into(&mut batch.texts[at].new);
                }
                Mutation::Attribute {
                    element,
                    name,
                    value,
                } => {
                    let NodeData::Element { attrs, .. } = &element.data else {
                        return None;
                    };
                    let at = match batch
                        .elements
                        .iter()
                        .position(|e| Rc::ptr_eq(&e.element, element))
                    {
                        Some(at) => at,
                        None => {
                            let old = attrs.borrow().clone();
                            batch.elements.push(AttrChange {
                                element: element.clone(),
                                new: old.clone(),
                                old,
                            });
                            batch.elements.len() - 1
                        }
                    };
                    set(&mut batch.elements[at].new, name, value.as_deref());
                }
            }
        }
        batch.texts.retain(|t| t.old != t.new);
        batch.elements.retain(|e| e.old != e.new);
        Some(batch)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.texts.is_empty() && self.elements.is_empty()
    }

    /// Записать итоговые значения в исходный DOM.
    pub(super) fn write(&self) {
        self.store(|t| &t.new, |e| &e.new);
    }

    /// Вернуть прежние значения: пакет отвергнут.
    pub(super) fn undo(&self) {
        self.store(|t| &t.old, |e| &e.old);
    }

    fn store(
        &self,
        text: impl Fn(&TextChange) -> &String,
        attrs: impl Fn(&AttrChange) -> &Vec<Attribute>,
    ) {
        for t in &self.texts {
            if let NodeData::Text { contents } = &t.node.data {
                contents.replace(text(t).as_str().into());
            }
        }
        for e in &self.elements {
            if let NodeData::Element { attrs: list, .. } = &e.element.data {
                list.replace(attrs(e).clone());
            }
        }
    }
}

/// `setAttribute`/`removeAttribute` (DOM §4.9): значение на месте прежнего
/// атрибута, новый — в конец списка.
fn set(list: &mut Vec<Attribute>, name: &QualName, value: Option<&str>) {
    let at = list.iter().position(|a| a.name == *name);
    match (at, value) {
        (Some(at), Some(value)) => list[at].value = value.into(),
        (None, Some(value)) => list.push(Attribute {
            name: name.clone(),
            value: value.into(),
        }),
        (Some(at), None) => {
            list.remove(at);
        }
        (None, None) => {}
    }
}

/// Записи, переводящие атрибуты `old` в список `new` с тем же порядком: общее
/// начало остаётся, хвост `old` снимается, хвост `new` ставится по порядку.
pub(super) fn attribute_records(
    element: &Handle,
    old: &[Attribute],
    new: &[Attribute],
) -> Vec<Mutation> {
    let common = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let record = |name: &QualName, value: Option<String>| Mutation::Attribute {
        element: element.clone(),
        name: name.clone(),
        value,
    };
    old[common..]
        .iter()
        .map(|a| record(&a.name, None))
        .chain(
            new[common..]
                .iter()
                .map(|a| record(&a.name, Some(a.value.to_string()))),
        )
        .collect()
}

/// Узел принадлежит DOM `root` и стоит под элементами HTML вне шаблонов и
/// чужого содержимого: у такого узла атрибуты и текст не часть формы
/// (`shape`). Текст требует родителя-элемента.
pub(super) fn open(node: &Handle, root: &Handle) -> bool {
    let mut at = node.clone();
    let mut first = true;
    loop {
        match &at.data {
            NodeData::Document => return Rc::ptr_eq(&at, root) && !first,
            NodeData::Element { name, .. } => {
                if name.ns != html5ever::ns!(html) || &*name.local == "template" {
                    return false;
                }
            }
            NodeData::Text { .. } if first => {}
            _ => return false,
        }
        let Some(parent) = parent(&at) else {
            return false;
        };
        if first
            && matches!(at.data, NodeData::Text { .. })
            && !matches!(parent.data, NodeData::Element { .. })
        {
            return false;
        }
        first = false;
        at = parent;
    }
}

fn parent(node: &Handle) -> Option<Handle> {
    let weak = node.parent.take();
    let parent = weak
        .as_ref()
        .and_then(markup5ever_rcdom::WeakHandle::upgrade);
    node.parent.set(weak);
    parent
}
