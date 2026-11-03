//! Отделение распорок полей от атомов при регистрации абзаца. Упорядоченные
//! позиции проходят слиянием без новой памяти; произвольный публичный ввод
//! использует временный индекс и сохраняет порядок распорок и дубликаты.

pub(super) fn remove_atom_spacers(
    spacers: &mut Vec<usize>,
    atoms: impl Iterator<Item = usize> + Clone,
) {
    let _queries = spacers.len();
    let mut _steps = 0;
    if atoms.clone().is_sorted() && spacers.is_sorted() {
        let mut atoms = atoms.peekable();
        spacers.retain(|s| {
            while atoms.peek().is_some_and(|at| at < s) {
                atoms.next();
                _steps += 1;
            }
            _steps += 1;
            atoms.peek() != Some(s)
        });
    } else {
        let mut atoms: Vec<_> = atoms.collect();
        atoms.sort_unstable();
        spacers.retain(|s| {
            atoms
                .binary_search_by(|at| {
                    _steps += 1;
                    at.cmp(s)
                })
                .is_err()
        });
    }
    crate::work::add!(ParagraphSpacerQueries, _queries);
    crate::work::add!(ParagraphSpacerSteps, _steps);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_registration_removes_atom_carriers_and_keeps_margin_duplicates() {
        let mut spacers = vec![0, 0, 3, 3, 6, 9, 12, 12, 15];
        remove_atom_spacers(&mut spacers, [3, 3, 9, 15, 21].into_iter());
        assert_eq!(spacers, [0, 0, 6, 12, 12]);
    }

    #[test]
    fn arbitrary_input_keeps_original_spacer_order() {
        let mut spacers = vec![12, 3, 0, 12, 9, 6, 3];
        remove_atom_spacers(&mut spacers, [9, 3, 9].into_iter());
        assert_eq!(spacers, [12, 0, 12, 6]);
        remove_atom_spacers(&mut spacers, [].into_iter());
        assert_eq!(spacers, [12, 0, 12, 6]);
        remove_atom_spacers(&mut Vec::new(), [0].into_iter());
    }
}
