//! Регрессии переноса атомов: приоритет слотов, щупы и запреты CSS Text.
use super::*;
use gpui::{IntoElement, div, px};

fn paragraph(text: &str, atoms: &[(usize, Option<(Option<char>, Option<char>)>)]) -> Paragraph {
    let mut p = Paragraph::new(
        text.to_owned().into(),
        vec![],
        px(16.),
        px(20.),
        Align::Left,
        Wrap::default(),
    )
    .atoms(
        atoms
            .iter()
            .map(|&(at, base_edges)| {
                (
                    at,
                    div().into_any_element(),
                    AtomAlign::Shift(0.),
                    RubyExtents {
                        base_edges,
                        ..Default::default()
                    },
                )
            })
            .collect(),
    );
    p.atom_boxes = atoms
        .iter()
        .map(|&(at, _)| AtomBox {
            at,
            h: 8.,
            base: 8.,
            align: AtomAlign::Shift(0.),
            over: 0.,
            under: 0.,
            shift: 0.,
        })
        .collect();
    p
}

#[test]
fn ruby_lookup_preserves_first_slot_and_empty_probe_semantics() {
    let p = paragraph(
        "\u{feff}\u{feff}x",
        &[
            (3, Some((Some('a'), Some('z')))),
            (0, None),
            (3, Some((Some('b'), None))),
        ],
    );
    assert_eq!(p.ruby_edges(3), Some((Some('a'), Some('z'))));
    assert_eq!(p.ruby_edges(0), None);
    assert_eq!(p.ruby_edges(1), None);
    let mut probe = paragraph("\u{feff}\u{feff}x", &[]);
    probe.atom_boxes = p.atom_boxes.clone();
    assert_eq!(probe.ruby_edges(3), None);
}

#[test]
fn atomic_wrap_keeps_gl_wj_zwj_and_nbsp_exception() {
    for (neighbour, allowed) in [
        ('x', true),
        (':', true),
        ('\u{a0}', true),
        ('\u{202f}', false),
        ('\u{2060}', false),
        ('\u{200d}', false),
    ] {
        let text = format!("{neighbour}\u{feff}{neighbour}");
        let start = neighbour.len_utf8();
        let p = paragraph(&text, &[(start, None)]);
        let breaks = p.linebreaks();
        assert_eq!(
            breaks.iter().any(|&(at, _)| at == start),
            allowed,
            "before {neighbour:?}"
        );
        assert_eq!(
            breaks.iter().any(|&(at, _)| at == start + 3),
            allowed,
            "after {neighbour:?}"
        );
    }
}

/// Ruby edges break like the base text: `<ruby>a<rt>x</rt></ruby>b` has
/// no opportunity, a space or CJK neighbours do.
#[test]
fn ruby_edges_follow_uax14_between_base_and_neighbour() {
    assert!(!super::pair_breaks('a', 'b'));
    assert!(super::pair_breaks(' ', 'a'));
    assert!(super::pair_breaks('漢', '字'));
    assert!(!super::pair_breaks('a', ','));
}
