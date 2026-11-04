//! Исходы перепроверяемых правил элемента для разделения стиля (Stylo
//! хранит их битовым вектором элемента): каждое правило сопоставляется с
//! элементом один раз, сколько бы записей кэша его ни проверяли.

use crate::dom::shadow::Scope;
use crate::style::select::matching::{matches, matches_ignoring_pseudo};
use crate::style::select::{Ancestor, Sibs};

/// Исход перепроверяемого правила: номер, `matches` и
/// `matches_ignoring_pseudo` (слои `:hover` и псевдоэлементов).
pub(super) type Check = (usize, bool, bool);

/// Псевдоклассы и псевдоэлементы субъекта, по которым `pseudo_styles`
/// собирает слои через `matches_ignoring_pseudo`. У прочих правил разделяемый
/// результат читает только `matches`.
const LAYERS: [&str; 4] = ["hover", "first-letter", "first-line", "marker"];

/// Исход правила `i` на элементе. Исход слоя считается только для правил
/// слоёв, у прочих он повторяет `matches`.
pub(super) fn outcome(
    memo: &mut Vec<Check>,
    scope: &Scope,
    i: usize,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> Check {
    if let Some(&known) = memo.iter().find(|c| c.0 == i) {
        return known;
    }
    let sel = &scope.rules[i].sel;
    let plain = matches(sel, me, path, sibs);
    let layer = match sel.pseudo.as_deref() {
        Some(p) if !plain && LAYERS.contains(&p) => matches_ignoring_pseudo(sel, me, path, sibs),
        _ => plain,
    };
    memo.push((i, plain, layer));
    (i, plain, layer)
}
