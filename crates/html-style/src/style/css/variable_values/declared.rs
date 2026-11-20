//! Computing only the custom properties an element declares itself.

use super::{Registered, Resolver, VarMap, Work};
use crate::budget::admission::Buffer;
use std::collections::{HashMap, HashSet};

/// The custom properties of an element from its cascaded map `cascaded`:
/// the parent's computed values with the element's own winning declarations
/// over them (`custom_properties::cascade_map`). Only the names `own` admits
/// are computed: a custom property inherits its computed value (CSS Variables
/// §2), so an inherited value is already substituted and passes through, as
/// Stylo shares the parent's map and computes only the declared properties
/// (`CustomPropertiesBuilder`). The others resolve their `var()` against the
/// whole map, inherited values included.
pub fn compute_declared(
    mut cascaded: VarMap,
    parent: &VarMap,
    registered: &HashMap<String, Registered>,
    accepts: impl Fn(&str, &str) -> bool,
    work: &Work<'_>,
    own: impl Fn(&str) -> bool,
) -> VarMap {
    let mut names: Vec<&String> = cascaded
        .keys()
        .filter(|name| name.starts_with("--") && own(name))
        .collect();
    names.sort_unstable();
    let mut resolver = Resolver {
        raw: &cascaded,
        parent,
        registered,
        accepts,
        stack: Buffer::default(),
        cyclic: HashSet::new(),
        cache: HashMap::new(),
    };
    let values: Vec<(String, Option<String>)> = names
        .into_iter()
        .map(|name| (name.clone(), resolver.value(name, work)))
        .collect();
    drop(resolver);
    for (name, value) in values {
        match value {
            Some(value) => {
                cascaded.insert(name, value);
            }
            None => {
                cascaded.remove(&name);
            }
        }
    }
    cascaded
}
