//! Prepare every tree scope before caching relational selector results.

use super::{
    Scope,
    assignment::{declarative_shadow, shadow_scope},
};
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
) -> Prepared {
    let mut scopes = HashMap::new();
    collect(document, agent, media, layers, args, &mut scopes);
    if !args.is_empty() {
        mark_has(document, args, &mut vec![], tree);
        for (root, _) in scopes.values() {
            mark_has(root, args, &mut vec![], tree);
        }
    }
    scopes
}

fn collect(
    node: &Handle,
    agent: &Scope,
    media: Media,
    layers: &mut LayerRegistry,
    args: &mut Vec<HasArg>,
    scopes: &mut Prepared,
) {
    if let Some((_, root)) = declarative_shadow(node) {
        let (scope, inner_args) = shadow_scope(&root, agent, media, layers);
        for arg in inner_args {
            if !args.iter().any(|a| a.id == arg.id) {
                args.push(arg);
            }
        }
        scopes.insert(NodeKey::of(node), (root.clone(), scope));
        collect(&root, agent, media, layers, args, scopes);
    }
    for child in node.children.borrow().iter() {
        collect(child, agent, media, layers, args, scopes);
    }
}
