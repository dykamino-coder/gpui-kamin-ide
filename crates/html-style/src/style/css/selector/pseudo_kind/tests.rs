//! Parsed pseudo-class kinds: each text maps to the branch matching used to
//! pick by prefix, and the kind is stored on the prepared argument.

use super::*;
use crate::style::css::Selector;

#[test]
fn texts_map_to_the_kind_matching_evaluates() {
    for (raw, kind) in [
        ("not(.a)", PseudoKind::Not),
        ("is(.a, .b)", PseudoKind::Is),
        ("where(.a)", PseudoKind::Is),
        ("any(.a)", PseudoKind::Is),
        ("host", PseudoKind::Host),
        ("host(.a)", PseudoKind::Host),
        ("has-slotted", PseudoKind::HasSlotted),
        ("link", PseudoKind::Link),
        ("visited", PseudoKind::Visited),
        ("dir(rtl)", PseudoKind::Dir(true)),
        ("dir( RTL )", PseudoKind::Dir(true)),
        ("dir(ltr)", PseudoKind::Dir(false)),
        ("root", PseudoKind::Root),
        ("empty", PseudoKind::Empty),
        ("target-current", PseudoKind::TargetCurrent),
        ("lang(de)", PseudoKind::Lang),
        ("hover", PseudoKind::UserAction),
        ("active", PseudoKind::UserAction),
        ("focus", PseudoKind::Other),
        (
            "first-child",
            PseudoKind::Structural(Position::First, false),
        ),
        ("last-of-type", PseudoKind::Structural(Position::Last, true)),
        ("only-child", PseudoKind::Structural(Position::Only, false)),
        (
            "nth-of-type(2n)",
            PseudoKind::Structural(Position::Nth, true),
        ),
        (
            "nth-last-child(1)",
            PseudoKind::Structural(Position::NthLast, false),
        ),
        // A functional name without its closing parenthesis is not that pseudo-class.
        ("not(.a", PseudoKind::Other),
        ("is(.a", PseudoKind::Other),
    ] {
        assert_eq!(PseudoKind::of(raw, false), kind, "{raw}");
    }
    assert_eq!(
        PseudoKind::of("has(> .a)", false),
        PseudoKind::Has(has_argument_id("> .a"))
    );
    assert_eq!(
        PseudoKind::of("nth-child(2n of .a)", true),
        PseudoKind::NthOf(false)
    );
    assert_eq!(
        PseudoKind::of("nth-last-child(1 of .a)", true),
        PseudoKind::NthOf(true)
    );
}

#[test]
fn parsed_selectors_carry_the_kind_of_every_pseudo_class() {
    let sel = Selector::parse("li:first-child:nth-child(2n of .x):has(> b)").unwrap();
    let kinds: Vec<_> = sel
        .pseudo
        .iter()
        .chain(&sel.also)
        .map(|p| sel.pseudo_of(p).1)
        .collect();
    assert!(kinds.contains(&PseudoKind::Structural(Position::First, false)));
    assert!(kinds.contains(&PseudoKind::NthOf(false)));
    assert!(kinds.contains(&PseudoKind::Has(has_argument_id("> b"))));
    assert!(
        sel.arguments
            .iter()
            .all(|p| p.kind == PseudoKind::of(&p.raw, p.nth.is_some()))
    );
}
