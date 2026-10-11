//! CSS coverage tests checks.

use super::*;

#[test]
fn no_field_is_written_and_never_read() {
    let dead = dead_fields();
    assert!(
        dead.is_empty(),
        "поля разрешённого стиля никто не читает — свойство числится              поддержанным, но на картинке его нет: {dead:?}"
    );
}

#[test]
fn every_mapped_property_reaches_the_style() {
    let broken = broken_promises();
    assert!(
        broken.is_empty(),
        "помечены перенесёнными, но разбор ничего не меняет: {broken:?}"
    );
}

#[test]
fn the_registry_has_no_duplicates() {
    let mut names: Vec<&str> = PROPERTIES.iter().map(|p| p.name).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "свойство перечислено дважды");
}

#[test]
fn coverage_does_not_regress() {
    let pct = mapped_pct();
    assert!(pct >= 80.0, "покрытие упало до {pct:.1}%");
}
