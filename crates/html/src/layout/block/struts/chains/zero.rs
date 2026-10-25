//! Применение поглощённых полей одним обходом общих префиксов путей.

use super::pins::pin_inherited_margins;
use crate::dom::Node;
use crate::style::values::value::Len;
use std::collections::BTreeMap;

#[derive(Default)]
struct Branch {
    next: BTreeMap<usize, usize>,
    deep: Option<bool>,
}

/// Арена путей: спуск добавляет ребро, не копирует весь путь до корня.
pub(crate) struct ZeroPaths(Vec<Branch>);
impl Default for ZeroPaths {
    fn default() -> Self {
        Self(vec![Branch::default()])
    }
}
impl ZeroPaths {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.len() == 1
    }
    pub(crate) fn add(&mut self, parent: usize, index: usize, deep: bool) -> usize {
        let next = self.0.len();
        self.0.push(Branch {
            deep: Some(deep),
            ..Default::default()
        });
        assert!(self.0[parent].next.insert(index, next).is_none());
        next
    }
}

pub(crate) fn zero_paths(
    children: &mut [Node],
    paths: &ZeroPaths,
    top: bool,
    vcx: crate::document::state::ValueCx<'_>,
) {
    // Повторное схлопывание уже обнулённой цепочки не должно отделять
    // все её списки COW и копировать стили каждого потомка заново.
    if !needs_write(children, paths, top) {
        return;
    }
    let mut pending = vec![(children, 0)];
    while let Some((children, branch)) = pending.pop() {
        for (index, node) in children.iter_mut().enumerate() {
            let Some(&next) = paths.0[branch].next.get(&index) else {
                continue;
            };
            let Node::Element(child) = node else { continue };
            if let Some(deep) = paths.0[next].deep {
                pin_inherited_margins(child, top || deep, !top || deep, vcx);
                if top || deep {
                    child.style.used_mut().margin.top = Some(Len::Px(0.0));
                }
                if !top || deep {
                    child.style.used_mut().margin.bottom = Some(Len::Px(0.0));
                }
            }
            if !paths.0[next].next.is_empty() {
                pending.push((&mut child.children, next));
            }
        }
    }
}

fn needs_write(children: &[Node], paths: &ZeroPaths, top: bool) -> bool {
    let mut pending = vec![(children, 0)];
    while let Some((children, branch)) = pending.pop() {
        for (&index, &next) in &paths.0[branch].next {
            let Node::Element(child) = &children[index] else {
                continue;
            };
            if let Some(deep) = paths.0[next].deep {
                let style = child.style.used();
                if (top || deep) && style.margin.top != Some(Len::Px(0.0))
                    || (!top || deep) && style.margin.bottom != Some(Len::Px(0.0))
                {
                    return true;
                }
                // Наследующие дети ещё могут требовать закрепления, даже
                // когда собственное поле уже равно нулю.
                if child.children.iter().any(|node| {
                    let Node::Element(e) = node else { return false };
                    [(0, top || deep), (2, !top || deep)]
                        .into_iter()
                        .any(|(side, wanted)| {
                            wanted
                                && e.style.cascaded().inherit_longhands.contains(
                                    crate::style::css::LonghandId::physical(
                                        crate::style::css::BoxGroup::Margin,
                                        side,
                                    ),
                                )
                        })
                }) {
                    return true;
                }
            }
            pending.push((&child.children, next));
        }
    }
    false
}
