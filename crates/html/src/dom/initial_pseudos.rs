//! Слой `::first-letter`/`::first-line`: правила псевдоэлемента поверх базы.
//!
//! `resolve` отбирает правила с заданным псевдоэлементом, чьи селекторы
//! совпадают с хозяином без учёта псевдо, и накладывает их по специфичности
//! и порядку на `base`. С базой — копией стиля элемента — выходит слой для
//! прямого пути текста; без базы — только авторские объявления, отдельно от
//! порождающей коробки (`first_letter_own`/`first_line_own`).

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::css::{Rule, VarMap};
use crate::style::select::matching::matches_ignoring_pseudo;
use crate::style::select::{Ancestor, Sibs};

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    name: &str,
    rules: &[Rule],
    vars: &VarMap,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    base: Option<&Computed>,
    vcx: ValueCx<'_>,
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
            merged.apply_decls_with_vars(&rule.decls, vars, vcx);
        }
        merged
    })
}
