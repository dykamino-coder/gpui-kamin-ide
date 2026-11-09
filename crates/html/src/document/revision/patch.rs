//! Правка атрибутов коробок адресуемых элементов в разделяемом дереве: копия
//! пути (`Children` копируется при записи) только до каждой правки.

use crate::dom::{Element, Node};
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
/// `None` — у коробки атрибуты не те, что дал прежний DOM (их переписал
/// проход сборки), и правка неприменима; пустой список — ни одна коробка не
/// несёт правленых атрибутов. Без целей дерево не обходится.
pub(super) fn attr_paths(
    nodes: &[Node],
    targets: &HashMap<u64, Target>,
) -> Option<Vec<(Path, u64)>> {
    let mut paths = Vec::new();
    if !targets.is_empty() && !find(nodes, targets, &mut paths) {
        return None;
    }
    Some(paths)
}

/// Записать атрибуты новой ревизии по путям `attr_paths` (копия только путей).
pub(super) fn patch(
    nodes: &mut Vec<Node>,
    paths: Vec<(Path, u64)>,
    targets: &HashMap<u64, Target>,
) -> bool {
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

/// Пути ко всем коробкам адресуемых элементов.
fn find(nodes: &[Node], targets: &HashMap<u64, Target>, paths: &mut Vec<(Path, u64)>) -> bool {
    elements(nodes, |e, path| match targets.get(&e.node_id) {
        Some(target) if e.tag == target.tag && e.attrs == target.old => {
            paths.push((path.to_vec(), e.node_id));
            true
        }
        Some(target) => e.tag != target.tag && e.attrs.is_empty(),
        None => true,
    })
}

/// Обход коробок в прямом порядке с путём к каждой (без рекурсии: глубина
/// дерева коробок ограничена только пределом представления); `false` от
/// `visit` прерывает обход, и тогда результат — `false`.
pub(super) fn elements(nodes: &[Node], mut visit: impl FnMut(&Element, &[usize]) -> bool) -> bool {
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
        if !visit(e, &path) {
            return false;
        }
        stack.push((&e.children, 0));
    }
    true
}
