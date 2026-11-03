//! Структурные псевдоклассы: :first/last/only-*, :nth-*(An+B [of S]).

use super::*;

/// `:nth-child(An+B of S)` / `:nth-last-child(An+B of S)` по сырому тексту
/// псевдокласса. `None` — псевдокласс не of-формы.
#[cfg(test)]
pub(super) fn nth_of_holds(
    sel: &Selector,
    pseudo: &str,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> Option<bool> {
    match sel.pseudo_of(pseudo) {
        (prepared, PseudoKind::NthOf(backwards)) => Some(nth_of_holds_prepared(
            sel, pseudo, prepared, backwards, me, path, sibs,
        )),
        _ => None,
    }
}

/// `:nth-child(An+B of S)` / `:nth-last-child(An+B of S)` (селекторы-4):
/// узел обязан сам совпасть с S, а номер считается только среди совпавших
/// братьев — с начала либо с конца (`backwards`).
pub(super) fn nth_of_holds_prepared(
    sel: &Selector,
    pseudo: &str,
    prepared: Option<&Prepared>,
    backwards: bool,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> bool {
    let Some(prepared) = prepared else {
        return false;
    };
    let Some(anb) = prepared.anb else {
        return false;
    };
    let list = &prepared.list;
    let hit = |a: &Ancestor, s: Sibs| list.iter().any(|sel| matches(sel, a, path, s));
    if !sibs.is_elem || list.is_empty() {
        return false;
    }
    // Состояния ссылок/маркеров и overlay меняются в пределах прохода.
    let stable = !me.matching.has_anchor() && prepared.nth_cacheable;
    if stable {
        let Some(ranks) = me
            .matching
            .nth_ranks(sel.argument_namespace as u8, pseudo, sibs, hit)
        else {
            // Отказ бюджета не превращает распознанную of-форму в обычную nth.
            return false;
        };
        let rank = ranks[sibs.pos];
        let index = if backwards {
            ranks[sibs.all.len()] + 1 - rank
        } else {
            rank
        };
        return rank > 0 && anb.matches(index);
    }
    if !hit(me, sibs) {
        return false;
    }
    let (peers, base) = if backwards {
        (sibs.next(), sibs.pos + 1)
    } else {
        (sibs.prev(), 0)
    };
    let idx = 1 + peers
        .iter()
        .enumerate()
        .filter(|(i, a)| hit(a, sibs.at(base + i)))
        .count();
    anb.matches(idx)
}

/// Структурные псевдоклассы по сырому тексту: место узла среди соседей.
/// `None` — псевдокласс не структурный, решение принимает вызывающий.
#[cfg(test)]
pub(super) fn structural(sel: &Selector, pseudo: &str, spot: Spot) -> Option<bool> {
    match sel.pseudo_of(pseudo) {
        (prepared, PseudoKind::Structural(position, of_type)) => {
            Some(structural_at(position, of_type, prepared, spot))
        }
        _ => None,
    }
}

/// Структурный псевдокласс разобранного вида: место узла среди соседей
/// (`of_type` — среди соседей с тем же тегом).
pub(super) fn structural_at(
    position: Position,
    of_type: bool,
    prepared: Option<&Prepared>,
    spot: Spot,
) -> bool {
    let (index, total) = if of_type {
        (spot.of_type, spot.of_type_total)
    } else {
        (spot.index, spot.total)
    };
    // Узел без места — не элемент; таким структурные правила не адресуются.
    if index == 0 {
        return false;
    }
    let anb_matches = |index| {
        prepared
            .and_then(|p| p.anb)
            .is_some_and(|a| a.matches(index))
    };
    match position {
        Position::First => index == 1,
        Position::Last => index == total,
        Position::Only => total == 1,
        Position::Nth => anb_matches(index),
        Position::NthLast => anb_matches(total + 1 - index),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_formulas_cover_forward_reverse_and_type_positions() {
        let spot = Spot {
            index: 2,
            total: 5,
            of_type: 1,
            of_type_total: 3,
        };
        for (text, expected) in [
            (":nth-child(even)", true),
            (":nth-last-child(4)", true),
            (":nth-of-type(-n+1)", true),
            (":nth-last-of-type(3)", true),
            (":nth-of-type(2)", false),
            (":nth-child(2n 1)", false),
            (":nth-of-type(1 of .x)", false),
        ] {
            let sel = Selector::parse(text).unwrap();
            assert_eq!(
                structural(&sel, sel.pseudo.as_deref().unwrap(), spot),
                Some(expected),
                "{text}"
            );
        }
    }

    #[test]
    fn rejected_filtered_ranks_never_fall_through_to_unfiltered_formula() {
        use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits, ResourceGovernor};
        let dom = crate::dom::xml::xml_dom("<r><x/></r>").unwrap();
        let doc = crate::values::DocumentValues::detached(Default::default());
        let owner =
            DocumentBudget::new(DocumentLimits::default(), ResourceGovernor::default()).unwrap();
        let revision = owner.revision();
        doc.admit_revision(revision.clone(), None);
        let tree = crate::dom::tree_index::TreeIndex::new(doc, true);
        let root = dom.document.children.borrow()[0].clone();
        let (_, all) = crate::style::select::census_of(&root.children.borrow(), &tree);
        let sibs = Sibs {
            all: &all,
            pos: 0,
            is_elem: true,
            rc: None,
        };
        let remaining = owner.limits().limit(BudgetKind::TraversalVisits)
            - revision.used(BudgetKind::TraversalVisits);
        revision
            .consume(BudgetKind::TraversalVisits, remaining)
            .unwrap();
        for text in [":nth-child(1 of x)", ":nth-last-child(1 of x)"] {
            let sel = Selector::parse(text).unwrap();
            let pseudo = sel.pseudo.as_deref().unwrap();
            assert_eq!(nth_of_holds(&sel, pseudo, &all[0], &[], sibs), Some(false));
            assert!(!pseudo_holds(&sel, pseudo, &all[0], &[], sibs));
        }
        assert!(revision.failure().is_some());
    }
}
