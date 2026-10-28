//! Группы строк, сведённые `orphans`/`widows` (`FlowGeometry::keep_lines`), — пожелание, а не
//! монолит: правило 3 css-break-3 §4.4 снимается первым, раньше запретов `break-*: avoid`
//! (Blink `kBreakAppealViolatingOrphansAndWidows`). Диапазон из двух и более строк отдаётся
//! планировщику как keep, сами строки остаются неделимыми, между ними — точки разрыва.

use crate::tolerance::COORD_EPS;

/// Solid ranges, cuts and keeps of measured line geometry.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Softened {
    pub(crate) solid: Vec<(f32, f32)>,
    pub(crate) cuts: Vec<(f32, f32)>,
    pub(crate) keep: Vec<(f32, f32)>,
}

/// Split each solid range of `solid` that holds two or more of `lines`
/// (`(top, bottom, consumed_before)`) into its lines and a keep.
pub(crate) fn soften(solid: &[(f32, f32)], lines: &[(f32, f32, f32)]) -> Softened {
    let mut out = Softened::default();
    for &(top, bottom) in solid {
        let mut inside: Vec<_> = lines
            .iter()
            .filter(|l| l.0 >= top - COORD_EPS && l.1 <= bottom + COORD_EPS)
            .collect();
        // The same line box may be recorded more than once (a paragraph and
        // its continuation): one line, the smallest consumed space before it.
        inside.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.total_cmp(&b.2)));
        inside.dedup_by(|a, b| (a.0 - b.0).abs() < COORD_EPS && (a.1 - b.1).abs() < COORD_EPS);
        if inside.len() < 2 {
            out.solid.push((top, bottom));
            continue;
        }
        out.keep.push((top, bottom));
        for pair in inside.windows(2) {
            // Overlapping boxes are not consecutive lines: no break between.
            if pair[1].0 >= pair[0].1 - COORD_EPS {
                out.cuts.push((pair[0].1 + pair[1].2, pair[1].0));
            }
        }
        out.solid
            .extend(inside.iter().map(|l| (l.0.max(top), l.1.min(bottom))));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kept_lines_become_a_keep_with_breaks_between_them() {
        let s = soften(
            &[(0., 100.), (100., 150.)],
            &[(0., 50., 0.), (50., 100., 0.), (100., 150., 0.)],
        );
        assert_eq!(s.solid, [(0., 50.), (50., 100.), (100., 150.)]);
        assert_eq!(s.cuts, [(50., 50.)]);
        assert_eq!(s.keep, [(0., 100.)]);
    }

    #[test]
    fn a_line_recorded_twice_is_one_line() {
        let s = soften(
            &[(0., 60.)],
            &[(0., 30., 0.), (0., 30., 25.), (30., 60., 0.)],
        );
        assert_eq!(s.cuts, [(30., 30.)]);
        assert_eq!(s.solid, [(0., 30.), (30., 60.)]);
    }

    #[test]
    fn a_monolith_without_lines_stays_solid() {
        let s = soften(&[(0., 80.)], &[(0., 18., 0.)]);
        assert_eq!(s.solid, [(0., 80.)]);
        assert!(s.cuts.is_empty() && s.keep.is_empty());
    }
}
