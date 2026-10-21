//! Слой `::first-letter`/`::first-line`: правила псевдоэлемента поверх базы.
//!
//! `resolve` отбирает правила с заданным псевдоэлементом, чьи селекторы
//! совпадают с хозяином без учёта псевдо, и накладывает их каскадом на
//! `base`. С базой — копией стиля элемента — выходит слой для
//! прямого пути текста; без базы — только авторские объявления, отдельно от
//! порождающей коробки (`first_letter_own`/`first_line_own`).

use crate::document::state::ValueCx;
use crate::style::cascade::vars::CascadeCx;
use crate::style::computed::Computed;
use crate::style::css::{DeclBlock, Rule, VarMap};
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
    let found: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.sel.pseudo.as_deref() == Some(name))
        .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs))
        .collect();
    // Каскад правил псевдоэлемента — тот же, что у элемента (css-cascade-5
    // §6.2): важное объявление раннего правила переживает обычное позднего
    // (`cascade-009a/b`).
    (!found.is_empty()).then(|| {
        let mut merged = base.cloned().unwrap_or_default();
        merged.cascade_onto(&found, &DeclBlock::new(), vars, CascadeCx::NONE, vcx);
        merged
    })
}
