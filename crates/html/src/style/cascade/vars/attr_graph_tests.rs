//! Глубокие зависимости атрибутов, циклы и fallback вычисляются без стека Rust.

use super::*;
use crate::document::state::DocState;

#[test]
fn typed_attribute_chain_exceeds_the_old_pass_limit() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let doc = DocState::new(Default::default());
            let mut attrs: Vec<(String, String)> = (0..4096)
                .map(|n| (format!("a{n}"), format!("attr(a{} type(*))", n + 1)))
                .collect();
            attrs.push(("a4096".into(), "12".into()));
            let cx = CascadeCx {
                attrs: &attrs,
                ..CascadeCx::NONE
            };
            assert_eq!(
                resolve_attrs("width", "attr(a0 px)", cx, doc.values()).trim(),
                "12px"
            );
            assert!(doc.revision_budget().failure().is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn only_cycle_members_fail_and_outer_fallback_is_selected() {
    let doc = DocState::new(Default::default());
    let attrs = [
        ("a".into(), "attr(b type(*), red)".into()),
        ("b".into(), "attr(a type(*), blue)".into()),
        ("safe".into(), "green".into()),
    ];
    let cx = CascadeCx {
        attrs: &attrs,
        ..CascadeCx::NONE
    };
    assert_eq!(
        resolve_attrs("color", "attr(a type(*), green)", cx, doc.values()).trim(),
        "green"
    );
    assert_eq!(
        resolve_attrs(
            "color",
            "attr(safe type(*), attr(a type(*)))",
            cx,
            doc.values()
        )
        .trim(),
        "green"
    );
    assert!(doc.revision_budget().failure().is_none());
}
