//! Keep authored initial-pseudo declarations separate from the originating box.
//! The existing direct text path also needs its legacy base-style layer.

use crate::style::computed::Computed;
use crate::style::css::{Decls, Rule};
use crate::style::select::matching::matches_ignoring_pseudo;
use crate::style::select::{Ancestor, Sibs};

pub(super) fn resolve(
    name: &str,
    rules: &[Rule],
    vars: &Decls,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    base: Option<&Computed>,
) -> Option<Computed> {
    let mut found: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.sel.pseudo.as_deref() == Some(name))
        .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs))
        .collect();
    found.sort_by_key(|r| (r.sel.specificity(), r.order));
    (!found.is_empty()).then(|| {
        let mut merged = base.cloned().unwrap_or_default();
        for rule in found {
            merged.apply_decls_with_vars(&rule.decls, vars);
        }
        merged
    })
}
