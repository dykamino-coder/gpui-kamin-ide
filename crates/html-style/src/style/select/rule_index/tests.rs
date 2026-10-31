//! Регрессии полноты индекса и порядка кандидатов.

use super::*;
#[test]
fn index_keeps_universal_and_necessary_subject_tokens() {
    for (raw, expected) in [
        (".a .b", Some((1, "b"))),
        ("#id.x", Some((0, "id"))),
        ("DIV[x]", Some((2, "div"))),
        ("[x]", Some((3, "x"))),
        (":is(.a,.b)", None),
    ] {
        let sel = Selector::parse(raw).unwrap();
        assert_eq!(token(&sel), expected);
    }
}
