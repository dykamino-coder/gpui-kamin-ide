//! Правка атрибутов коробок адресуемых элементов в разделяемом дереве: копия
//! пути (`Children` копируется при записи) только до каждой правки.

use crate::dom::Node;
use std::collections::HashMap;

pub(super) type Attrs = Vec<(String, String)>;

/// Правка коробок одного исходного элемента: его тег и атрибуты в обеих ревизиях.
pub(super) struct Target {
    pub(super) tag: String,
    pub(super) old: Attrs,
    pub(super) new: Attrs,
}

/// Атрибуты новой ревизии у каждого узла коробок адресуемого элемента: у
/// коробок с его `node_id` и тегом (копии коробки делят оба). Синтетическая
/// коробка с тем же номером и другим тегом (`html-viewport` берёт следующий
/// за наибольшим номер дерева, которым может оказаться номер развёрнутого
/// `html`) атрибутов автора не несёт: она должна быть без атрибутов и не
/// правится. Элемент без коробки (`html`, `display: none`) правки не требует.
/// `false` — у коробки атрибуты не те, что дал прежний DOM (их переписал
/// проход сборки), и правка неприменима.
pub(super) fn patch(nodes: &mut Vec<Node>, targets: &HashMap<u64, Target>) -> bool {
    let mut paths = Vec::new();
    if !find(nodes, targets, &mut paths) {
        return false;
    }
    for (path, id) in paths {
        match (apply(nodes, &path), targets.get(&id)) {
            (Some(Node::Element(e)), Some(target)) => e.attrs = target.new.clone(),
            _ => return false,
        }
    }
    true
}

/// Номера детей от корня до узла дерева коробок.
pub(super) type Path = Vec<usize>;

/// Узел по пути `path`; копируются только списки детей на пути.
pub(super) fn apply<'a>(nodes: &'a mut Vec<Node>, path: &[usize]) -> Option<&'a mut Node> {
    let (last, parents) = path.split_last()?;
    let mut list: &mut Vec<Node> = nodes;
    for &i in parents {
        let Some(Node::Element(e)) = list.iter_mut().nth(i) else {
            return None;
        };
        list = &mut e.children;
    }
    list.iter_mut().nth(*last)
}

/// Пути ко всем коробкам адресуемых элементов (обход без рекурсии: глубина
/// дерева коробок ограничена только пределом представления).
fn find(nodes: &[Node], targets: &HashMap<u64, Target>, paths: &mut Vec<(Path, u64)>) -> bool {
    let mut stack: Vec<(&[Node], usize)> = vec![(nodes, 0)];
    let mut path = Vec::new();
    while let Some((list, next)) = stack.last_mut() {
        let list: &[Node] = list;
        let Some(node) = list.get(*next) else {
            stack.pop();
            path.pop();
            continue;
        };
        let index = *next;
        *next += 1;
        let Node::Element(e) = node else { continue };
        path.push(index);
        if let Some(target) = targets.get(&e.node_id) {
            if e.tag == target.tag && e.attrs == target.old {
                paths.push((path.clone(), e.node_id));
            } else if e.tag == target.tag || !e.attrs.is_empty() {
                return false;
            }
        }
        stack.push((&e.children, 0));
    }
    true
}
