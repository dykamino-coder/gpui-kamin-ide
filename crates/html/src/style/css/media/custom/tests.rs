//! Граф псевдонимов: глубина, логика зависимых запросов и участники цикла.

use super::*;
use crate::document::state::DocState;

#[test]
fn deep_aliases_use_heap_frames() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let doc = DocState::new(Media::default());
            let mut raw: HashMap<_, _> = (0..4096)
                .map(|n| (format!("--m{n:04}"), format!("(--m{:04})", n + 1)))
                .collect();
            raw.insert("--m4096".into(), "screen".into());
            let values = resolve_custom_media(raw, Media::default(), doc.values()).unwrap();
            assert_eq!(values.len(), 4097);
            assert!(values.values().all(|v| *v == SupTri::True));
            assert!(doc.revision_budget().failure().is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn cycle_members_remain_unknown_under_negation() {
    let doc = DocState::new(Media::default());
    let raw = custom_media_of(
        "@custom-media --a (--b); @custom-media --b not (--a);
         @custom-media --c not (--a); @custom-media --last false;
         @custom-media --last true;",
    );
    let values = resolve_custom_media(raw, Media::default(), doc.values()).unwrap();
    assert!(values["--a"] == SupTri::Unknown);
    assert!(values["--b"] == SupTri::Unknown);
    assert!(values["--c"] == SupTri::False);
    assert!(values["--last"] == SupTri::True);
}

#[test]
fn branching_cycle_includes_a_finished_dfs_branch() {
    let doc = DocState::new(Media::default());
    let raw = custom_media_of(
        "@custom-media --a (--b) or (--c); @custom-media --b (--a);
         @custom-media --c not (--b); @custom-media --d not (--missing);
         @custom-media --e not (--no); @custom-media --no false;",
    );
    let values = resolve_custom_media(raw, Media::default(), doc.values()).unwrap();
    for name in ["--a", "--b", "--c"] {
        assert!(values[name] == SupTri::Unknown);
    }
    assert!(values["--d"] == SupTri::False);
    assert!(values["--e"] == SupTri::True);
}
