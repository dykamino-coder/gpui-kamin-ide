//! Сопоставление селектора с узлом (`matches`): паспорт узла `Ancestor`,
//! цепочка предков и братья `Sibs` на входе, совпадение на выходе. Здесь —
//! псевдоклассы, зависящие от дерева и атрибутов (`:not()`, `:is()`,
//! `:root`, `:link`/`:visited`, `:dir()`, `:lang()`, `:empty`, `:has()`,
//! структурные `:nth-*`), затем компаунд, соседние и предковые комбинаторы.
//! `:hover`, `::before` и прочие состояния применяет отдельный слой отрисовки.

use crate::dom::language;
use crate::style::css::{Position, Prepared, PseudoKind, Selector};
use crate::style::select::{Ancestor, Sibs, Spot};

mod compound;
mod featureless;
mod nth;
mod pseudo;
use compound::{ancestor_holds, matches_compound};
use nth::{nth_of_holds_prepared, structural_at};
use pseudo::{host_holds, is_holds, pseudo_holds};

/// Сопоставление селектора с узлом и его цепочкой предков.
///
/// `sibs` — предыдущие соседи-элементы узла в порядке разметки: по ним
/// решаются соседние комбинаторы `+` и `~`.
pub fn matches(sel: &Selector, me: &Ancestor, path: &[Ancestor], sibs: Sibs) -> bool {
    crate::work::add!(StyleMatchAttempts, 1);
    if !me.matching.work(1) {
        return false;
    }
    // Безликий хост (изнутри своей тени): предков и братьев у него в этой
    // области нет, совпадает лишь `:host`-компаунд (см. `matches_compound`).
    if me.featureless.is_some() {
        return featureless::matches_featureless(sel, me).unwrap_or(false);
    }
    // The subject compound's pseudo-class is decided by its parsed kind; the
    // states applied by other layers (`:hover`, `::before`) never hold here.
    if let Some(pseudo) = &sel.pseudo
        && !pseudo_holds(sel, pseudo, me, path, sibs)
    {
        return false;
    }
    matches_ignoring_pseudo(sel, me, path, sibs)
}

/// То же сопоставление, но без отсева по псевдоклассу — для слоя наведения.
pub fn matches_ignoring_pseudo(
    sel: &Selector,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> bool {
    if !matches_compound(sel, me) {
        return false;
    }
    // Дополнительные псевдоклассы компаунда (`li:first-child:last-child`)
    // обязаны выполниться ВСЕ; раньше выживал только последний.
    if !sel
        .also
        .iter()
        .all(|p| pseudo_holds(sel, p, me, path, sibs))
    {
        return false;
    }
    // Соседний комбинатор: `+` — ровно предыдущий сосед-элемент, `~` — любой
    // раньше. Сосед проверяется ПОЛНЫМ сопоставлением со своими соседями
    // слева и тем же путём предков (соседи его делят).
    if let Some(prev) = &sel.prev {
        let (prev_sel, adjacent) = (&prev.0, prev.1);
        let prev = sibs.prev();
        let hit = |i: usize| matches(prev_sel, &prev[i], path, sibs.at(i));
        let found = if adjacent {
            !prev.is_empty() && hit(prev.len() - 1)
        } else {
            (0..prev.len()).rev().any(hit)
        };
        if !found {
            return false;
        }
    }
    let Some(anc) = &sel.ancestor else {
        return true;
    };
    let (parent_sel, direct) = (&anc.0, anc.1);
    if direct {
        return !path.is_empty() && ancestor_holds(parent_sel, path, path.len() - 1);
    }
    (0..path.len())
        .rev()
        .any(|i| ancestor_holds(parent_sel, path, i))
}
