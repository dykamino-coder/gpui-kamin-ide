//! Проверки итогового списка точек разрыва: одна сборка на подготовку и
//! сохранённые точки части абзаца.

use super::*;
use gpui::px;

fn paragraph(text: &str, wrap: Wrap) -> Paragraph {
    Paragraph::new(
        text.to_owned().into(),
        vec![],
        px(16.),
        px(20.),
        Align::Left,
        wrap,
    )
}

fn at(p: &Paragraph, hyphenate: bool) -> Vec<(usize, bool)> {
    p.opportunities_with_fallback(hyphenate)
        .map(|s| (s.at, s.mandatory))
        .collect()
}

#[test]
fn opportunities_are_found_once_per_preparation() {
    let p = paragraph("aa bb\ncc dd", Wrap::default());
    let uncached = |p: &Paragraph, hyphenate| -> Vec<(usize, bool)> {
        p.opportunities_uncached(hyphenate)
            .iter()
            .map(|s| (s.at, s.mandatory))
            .collect()
    };
    assert_eq!(at(&p, false), [(3, false), (6, true), (9, false)]);
    assert!(
        p.prepared().stops.get().is_some(),
        "kept by the preparation"
    );
    assert_eq!(at(&p, false), uncached(&p, false));
    assert_eq!(at(&p, true), uncached(&p, true));
    // Wrap rules and spacers are inputs of the preparation: a paragraph
    // built with others breaks anew.
    let spaced = paragraph("aa bb\ncc dd", Wrap::default()).spacers(vec![3]);
    assert_eq!(at(&spaced, false), uncached(&spaced, false));
    let all = paragraph(
        "aa bb\ncc dd",
        Wrap {
            break_all: true,
            ..Default::default()
        },
    );
    assert_eq!(at(&all, false), uncached(&all, false));
    assert_ne!(at(&all, false), at(&p, false));
}

#[test]
fn a_part_takes_the_kept_opportunities_inside_it() {
    let p = paragraph("aa bb\ncc dd ee", Wrap::default());
    let all: Vec<(usize, bool)> = at(&p, false);
    for part in [0..14, 0..3, 3..9, 4..12, 9..9, 13..14] {
        let expected: Vec<_> = all
            .iter()
            .copied()
            .filter(|&(s, _)| s > part.start && s <= part.end)
            .collect();
        let stops = p.opportunities_in(&part);
        let got: Vec<_> = (0..stops.len())
            .filter_map(|i| stops.get(i))
            .map(|s| (s.at, s.mandatory))
            .collect();
        assert_eq!(got, expected, "{part:?}");
    }
}
