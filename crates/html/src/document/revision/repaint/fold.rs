//! Каскад коробок под снятыми обёртками `<html>`/`<body>`
//! (`document::unwrap_document`): проба даёт каскад элемента как обход, а
//! коробка `body` под снятым `html` и коробки детей снятого `body` несут
//! значения обёртки (`inherit_unwrapped`). Здесь та же сводка для пробы, чтобы
//! правка на месте давала то же дерево, что свежая сборка.

use crate::dom::Node;
use crate::style::cascade::inherit::inherit_unwrapped;
use crate::style::computed::{BoxStyle, Computed};
use crate::values::ValueCx;
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

/// Какие обёртки дерево коробок сняло.
#[derive(Clone, Copy)]
struct Folded {
    html: bool,
    body: bool,
}

impl Folded {
    fn of(nodes: &[Node]) -> Self {
        Self {
            html: !has_box(nodes, "html"),
            body: !super::paths::wrapped(nodes),
        }
    }

    fn any(self) -> bool {
        self.html || self.body
    }
}

/// Снятые обёртки среди целей пробы: их каскад сводится в коробки детей.
pub(super) struct Unwrapped {
    folded: Folded,
    wrappers: [Handle; 2],
    /// Номера `html` и `body` среди целей.
    at: [usize; 2],
}

/// Последняя снятая обёртка и значения, с которыми её дети стали коробками.
pub(super) struct Parent {
    handle: Handle,
    wrapper: BoxStyle,
}

impl Unwrapped {
    /// Обёртки, которые сняло дерево `nodes`, — в цели пробы `targets`;
    /// `Some(None)` — снятых обёрток нет, `None` — их нет в DOM.
    pub fn add(
        nodes: &[Node],
        document: &Handle,
        targets: &mut Vec<Handle>,
    ) -> Option<Option<Self>> {
        let folded = Folded::of(nodes);
        if !folded.any() {
            return Some(None);
        }
        let (html, body) = wrappers(document)?;
        let mut at = |h: &Handle| match targets.iter().position(|t| Rc::ptr_eq(t, h)) {
            Some(i) => i,
            None => {
                targets.push(h.clone());
                targets.len() - 1
            }
        };
        let at = [at(&html), at(&body)];
        Some(Some(Self {
            folded,
            wrappers: [html, body],
            at,
        }))
    }

    /// Цель `i` — снятая обёртка: её каскад пакет менять не должен (коробки
    /// у неё нет, её значения — у детей и у текста страницы).
    pub fn folds(&self, i: usize) -> bool {
        (self.folded.html && i == self.at[0]) || (self.folded.body && i == self.at[1])
    }

    /// Родитель коробок верхнего уровня по каскадам `styles` целей.
    pub fn parent<'a>(
        &self,
        styles: impl Fn(usize) -> Option<&'a Computed>,
        vcx: ValueCx<'_>,
    ) -> Option<Parent> {
        let (html, body) = (styles(self.at[0])?, styles(self.at[1])?);
        Some(Parent {
            handle: self.wrappers[usize::from(self.folded.body)].clone(),
            wrapper: parent(self.folded, html, body, vcx),
        })
    }
}

impl Parent {
    /// Каскад коробки цели `target` с каскадом элемента `own`.
    pub fn fold(&self, target: &Handle, own: Box<Computed>, vcx: ValueCx<'_>) -> Box<Computed> {
        if under(target, &self.handle) {
            child_box(&self.wrapper, &own, vcx)
        } else {
            own
        }
    }
}

/// Коробка с тегом `tag` в трёх верхних уровнях дерева.
fn has_box(nodes: &[Node], tag: &str) -> bool {
    let mut level: Vec<&Node> = nodes.iter().collect();
    for _ in 0..3 {
        let mut next = Vec::new();
        for node in level {
            if let Node::Element(e) = node {
                if e.tag == tag && e.node_id != 0 {
                    return true;
                }
                next.extend(e.children.iter());
            }
        }
        level = next;
    }
    false
}

fn element(handle: &Handle, name: &str) -> bool {
    matches!(&handle.data, NodeData::Element { name: n, .. } if &*n.local == name)
}

fn child(parent: &Handle, name: &str) -> Option<Handle> {
    let children = parent.children.borrow();
    children.iter().find(|c| element(c, name)).cloned()
}

/// `html` и его `body` исходного документа.
fn wrappers(document: &Handle) -> Option<(Handle, Handle)> {
    let html = child(document, "html")?;
    let body = child(&html, "body")?;
    Some((html, body))
}

/// Родитель элемента — `wrapper`.
fn under(handle: &Handle, wrapper: &Handle) -> bool {
    let parent = handle.parent.take();
    let under = parent
        .as_ref()
        .and_then(|p| p.upgrade())
        .is_some_and(|p| Rc::ptr_eq(&p, wrapper));
    handle.parent.set(parent);
    under
}

/// Последняя снятая обёртка и значения, которые получают каскады её детей:
/// `unwrap_document` снимает `html`, затем `body` (тот — со значениями `html`).
fn parent(folded: Folded, html: &Computed, body: &Computed, vcx: ValueCx<'_>) -> BoxStyle {
    let html = BoxStyle::from(html.clone());
    if folded.body {
        BoxStyle::from(inherit_unwrapped(&html, body, vcx))
    } else {
        html
    }
}

/// Каскад коробки ребёнка снятой обёртки со значениями `wrapper`.
fn child_box(wrapper: &BoxStyle, own: &Computed, vcx: ValueCx<'_>) -> Box<Computed> {
    Box::new(inherit_unwrapped(wrapper, own, vcx))
}
