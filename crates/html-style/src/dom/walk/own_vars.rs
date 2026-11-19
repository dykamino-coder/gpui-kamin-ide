//! The custom properties of an element during the walk: its parent's computed
//! map with its own declarations computed over it.

use crate::dom::tree_index::TreeIndex;
use crate::dom::*;
use crate::style::cascade::map::CascadeMap;
use crate::style::css::{DeclBlock, Rule, VarMap};
use std::rc::Rc;

/// `custom` says whether a matched block or the inline style declares a
/// `--*` name; `pre` is the element's cascade map when it was built.
///
/// An element without its own custom declarations has its parent's custom
/// properties while every registered one inherits and the parent has a value
/// for each that has an initial value (`cascade_map` would give the same
/// map): it shares the parent's map (Stylo; Blink `StyleInheritedVariables`).
/// Otherwise only its declared and registered names are computed
/// (`variable_values::compute_declared`).
pub(super) fn own_vars(
    matched: &[&Rule],
    inline_decls: &DeclBlock,
    custom: bool,
    pre: Option<&CascadeMap>,
    inherited: &Rc<VarMap>,
    tree: &TreeIndex,
    vcx: crate::values::ValueCx<'_>,
) -> Rc<VarMap> {
    let registered = tree.doc.property_rules();
    let inherits_all = registered
        .rules
        .iter()
        .all(|(name, r)| r.inherits && (r.initial.is_none() || inherited.contains_key(name)));
    if !custom && inherits_all {
        return inherited.clone();
    }
    let no_custom = CascadeMap::default();
    let cascaded = crate::style::css::custom_properties::cascade_map(
        pre.unwrap_or(&no_custom),
        inherited,
        &registered.rules,
        |s, v| syntax_accepts(s, v, vcx),
    );
    let declared: std::collections::HashSet<&str> = matched
        .iter()
        .map(|r| &r.decls)
        .chain(std::iter::once(inline_decls))
        .flat_map(|block| block.iter())
        .map(|d| d.name.as_str())
        .filter(|name| name.starts_with("--"))
        .collect();
    let revision = tree.doc.revision_budget();
    let work = crate::style::css::variable_values::Work::new(&revision);
    let own = crate::style::css::variable_values::compute_declared(
        cascaded,
        inherited,
        &registered.rules,
        |s, v| syntax_accepts(s, v, vcx),
        &work,
        |name| declared.contains(name) || registered.rules.contains_key(name),
    );
    work.finish(&tree.doc);
    Rc::new(own)
}
