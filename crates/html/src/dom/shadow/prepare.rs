//! Предпроход по всем областям дерева до кеширования отметок `:has()`.
//!
//! `prepare` находит каждую декларативную тень (включая вложенные), строит ей
//! область стилей (`shadow_scope`) и собирает аргументы `:has()` отовсюду:
//! аргумент из таблицы тени может смотреть в обычное дерево хоста, поэтому
//! сперва открываются все таблицы, затем отметки ставятся по всем деревьям,
//! и только потом снимаются переписи братьев. Выход — `Prepared`: хост → корень
//! тени и её область.

use super::{
    Scope,
    assignment::{declarative_shadow, shadow_scope},
};
use crate::document::state::ValueCx;
use crate::dom::tree_index::{NodeKey, TreeIndex};
use crate::style::css::{LayerRegistry, Media};
use crate::style::select::has::{HasArg, mark_has};
use markup5ever_rcdom::Handle;
use std::{collections::HashMap, rc::Rc};

pub(crate) type Prepared = HashMap<NodeKey, (Handle, Rc<Scope>)>;

/// Arguments from a shadow sheet can inspect the host's ordinary DOM tree.
/// Discover all sheets first, then mark all trees before census snapshots exist.
pub(crate) fn prepare(
    document: &Handle,
    agent: &Scope,
    media: Media,
    layers: &mut LayerRegistry,
    args: &mut Vec<HasArg>,
    tree: &mut TreeIndex,
) -> Result<Prepared, crate::budget::admission::Error> {
    let vcx = tree.doc.values();
    let mut scopes = HashMap::new();
    collect(document, agent, media, layers, args, &mut scopes, vcx)?;
    if !args.is_empty() {
        mark_has(document, args, &mut vec![], tree)?;
        for (root, _) in scopes.values() {
            mark_has(root, args, &mut vec![], tree)?;
        }
    }
    Ok(scopes)
}

fn collect(
    node: &Handle,
    agent: &Scope,
    media: Media,
    layers: &mut LayerRegistry,
    args: &mut Vec<HasArg>,
    scopes: &mut Prepared,
    vcx: ValueCx<'_>,
) -> Result<(), crate::budget::admission::Error> {
    let admission = vcx
        .doc
        .budget
        .borrow()
        .clone()
        .map(crate::budget::admission::Admission::new)
        .unwrap_or_default();
    let mut pending = vec![node.clone()];
    while let Some(node) = pending.pop() {
        admission.work(node.children.borrow().len() + 1)?;
        pending.extend(node.children.borrow().iter().rev().cloned());
        if let Some((_, root)) = declarative_shadow(&node) {
            let (scope, inner_args) = shadow_scope(&root, agent, media, layers, vcx)?;
            for arg in inner_args {
                admission.work(args.len())?;
                if !args.iter().any(|a| a.id == arg.id) {
                    args.push(arg);
                }
            }
            scopes.insert(NodeKey::of(&node), (root.clone(), scope));
            pending.push(root);
        }
    }
    Ok(())
}
