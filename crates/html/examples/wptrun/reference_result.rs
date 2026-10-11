//! Exact positive and negative reference relations share the validated RGB oracle.
//! A malformed capture never satisfies a negative reference by accident.

use super::pixel_compare;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Match,
    Mismatch,
}

pub fn same_file(a: &str, b: &str) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy()),
        _ => a.eq_ignore_ascii_case(b),
    }
}

pub fn verdict(a: &(u32, u32, Vec<u8>), b: &(u32, u32, Vec<u8>), relation: Relation) -> String {
    match (pixel_compare::compare(a, b), relation) {
        (None, _) => "invalid screenshot dimensions".into(),
        (Some(d), Relation::Mismatch) if d.pixels == 0 => "совпал с анти-эталоном".into(),
        (Some(_), Relation::Mismatch) => "0.00".into(),
        (_, Relation::Match) => pixel_compare::verdict(a, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_channel_difference_satisfies_only_negative_reference() {
        let a = (1, 1, vec![0, 0, 0, 255]);
        let b = (1, 1, vec![0, 1, 0, 255]);
        assert_eq!(verdict(&a, &b, Relation::Mismatch), "0.00");
        assert_eq!(
            verdict(&a, &b, Relation::Match),
            "pixel mismatch max=1 total=1"
        );
    }

    #[test]
    fn alpha_difference_does_not_satisfy_negative_reference() {
        let a = (1, 1, vec![1, 2, 3, 255]);
        let b = (1, 1, vec![1, 2, 3, 0]);
        assert_eq!(verdict(&a, &b, Relation::Match), "0.00");
        assert_eq!(
            verdict(&a, &b, Relation::Mismatch),
            "совпал с анти-эталоном"
        );
    }

    #[test]
    fn invalid_dimensions_or_buffers_fail_both_relations() {
        let a = (2, 1, vec![0; 8]);
        for b in [(1, 2, vec![0; 8]), (2, 1, vec![0; 4]), (0, 0, vec![])] {
            for relation in [Relation::Match, Relation::Mismatch] {
                assert_eq!(verdict(&a, &b, relation), "invalid screenshot dimensions");
            }
        }
    }
}
