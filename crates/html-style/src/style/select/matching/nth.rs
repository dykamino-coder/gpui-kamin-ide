//! Структурные псевдоклассы: :first/last/only-*, :nth-*(An+B [of S]).

use super::*;

/// Псевдокласс — of-форма `:nth-child(… of S)`?
pub(super) fn nth_of_form(sel: &Selector, pseudo: &str) -> bool {
    sel.prepared(pseudo).is_some_and(|p| p.nth.is_some())
}

/// `:nth-child(An+B of S)` / `:nth-last-child(An+B of S)` (селекторы-4):
/// узел обязан сам совпасть с S, а номер считается только среди совпавших
/// братьев — с начала либо с конца. `None` — псевдокласс не of-формы.
pub(super) fn nth_of_holds(
    sel: &Selector,
    pseudo: &str,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> Option<bool> {
    let (name, _arg) = pseudo.split_once('(')?;
    let backwards = match name {
        "nth-child" => false,
        "nth-last-child" => true,
        _ => return None,
    };
    let prepared = sel.prepared(pseudo)?;
    prepared.nth.as_ref()?;
    let Some(anb) = prepared.anb else {
        return Some(false);
    };
    let list = &prepared.list;
    let hit = |a: &Ancestor, s: Sibs| list.iter().any(|sel| matches(sel, a, path, s));
    if !sibs.is_elem || list.is_empty() {
        return Some(false);
    }
    // Состояния ссылок/маркеров и overlay меняются в пределах прохода.
    let stable = !me.matching.has_anchor() && prepared.nth_cacheable;
    if stable {
        let ranks = me
            .matching
            .nth_ranks(sel.argument_namespace as u8, pseudo, sibs, hit)?;
        let rank = ranks[sibs.pos];
        let index = if backwards {
            ranks[sibs.all.len()] + 1 - rank
        } else {
            rank
        };
        return Some(rank > 0 && anb.matches(index));
    }
    if !hit(me, sibs) {
        return Some(false);
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
    Some(anb.matches(idx))
}

/// Структурные псевдоклассы: место узла среди соседей.
///
/// `None` — псевдокласс не структурный, решение принимает вызывающий.
pub(super) fn structural(sel: &Selector, pseudo: &str, spot: Spot) -> Option<bool> {
    let name = pseudo.split_once('(').map_or(pseudo, |(name, _)| name);
    let anb_matches = |index| {
        sel.prepared(pseudo)
            .and_then(|p| p.anb)
            .is_some_and(|a| a.matches(index))
    };
    let (index, total) = match name {
        "first-child" | "last-child" | "only-child" | "nth-child" | "nth-last-child" => {
            (spot.index, spot.total)
        }
        "first-of-type" | "last-of-type" | "only-of-type" | "nth-of-type" | "nth-last-of-type" => {
            (spot.of_type, spot.of_type_total)
        }
        _ => return None,
    };
    // Узел без места — не элемент; таким структурные правила не адресуются.
    if index == 0 {
        return Some(false);
    }
    Some(match name {
        "first-child" | "first-of-type" => index == 1,
        "last-child" | "last-of-type" => index == total,
        "only-child" | "only-of-type" => total == 1,
        "nth-child" | "nth-of-type" => anb_matches(index),
        "nth-last-child" | "nth-last-of-type" => anb_matches(total + 1 - index),
        _ => false,
    })
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
}
