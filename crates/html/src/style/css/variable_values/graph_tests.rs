//! Глубокий граф var() и выбранные циклы на малом стеке потока.

use super::*;

#[test]
fn four_thousand_dependencies_use_heap_frames() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let mut raw = VarMap::new();
            for n in 0..4096 {
                raw.insert(format!("--v{n:04}"), format!("var(--v{:04})", n + 1));
            }
            raw.insert("--v4096".into(), "green".into());
            let revision = crate::budget::RevisionBudget::standalone();
            let work = Work::new(&revision);
            let values = compute(&raw, &VarMap::new(), &HashMap::new(), |_, _| true, &work);
            assert_eq!(values["--v0000"].trim(), "green");
            assert_eq!(values.len(), 4097);
            assert!(revision.failure().is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn cycle_invalidates_its_members_but_incoming_value_can_select_fallback() {
    let raw = VarMap::from([
        ("--a".into(), "var(--b, red)".into()),
        ("--b".into(), "var(--a, blue)".into()),
        ("--c".into(), "var(--a, green)".into()),
        ("--safe".into(), "green".into()),
        ("--unused".into(), "var(--safe, var(--unused))".into()),
    ]);
    let revision = crate::budget::RevisionBudget::standalone();
    let work = Work::new(&revision);
    let values = compute(&raw, &VarMap::new(), &HashMap::new(), |_, _| true, &work);
    assert!(!values.contains_key("--a"));
    assert!(!values.contains_key("--b"));
    assert_eq!(values["--c"].trim(), "green");
    assert_eq!(values["--unused"].trim(), "green");
    assert!(revision.failure().is_none());
}
