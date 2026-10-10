//! Tests for lang_case; split out to keep the owning module within 250 lines.

use super::*;

#[test]
fn turkic_and_lithuanian_follow_special_casing() {
    assert_eq!(upper("i ı", Tailoring::Turkic), "İ I");
    assert_eq!(lower("İ I\u{307} I", Tailoring::Turkic), "i i ı");
    assert_eq!(
        lower("Ì Í Ĩ", Tailoring::Lithuanian),
        "i\u{307}\u{300} i\u{307}\u{301} i\u{307}\u{303}"
    );
    assert_eq!(upper("i\u{307}\u{300}", Tailoring::Lithuanian), "I\u{300}");
}

#[test]
fn greek_uppercase_drops_accents() {
    assert_eq!(greek_upper("καλημέρα αύριο"), "ΚΑΛΗΜΕΡΑ ΑΥΡΙΟ");
    assert_eq!(greek_upper("θεϊκό"), "ΘΕΪΚΟ");
    assert_eq!(greek_upper("ευφυΐα Νεράιδα"), "ΕΥΦΥΪΑ ΝΕΡΑΪΔΑ");
    assert_eq!(greek_upper("ήσουν ή εγώ ή εσύ"), "ΗΣΟΥΝ Ή ΕΓΩ Ή ΕΣΥ");
}

#[test]
fn dutch_ij_and_tags() {
    let mut s = String::new();
    assert_eq!(
        title_start('i', &['j', 's'], Tailoring::Dutch, &mut s),
        Some(1)
    );
    assert_eq!(s, "IJ");
    assert_eq!(tailoring(Some("NL")), Some(Tailoring::Dutch));
    assert_eq!(tailoring(Some("tr-TR")), Some(Tailoring::Turkic));
    assert_eq!(tailoring(Some("en")), None);
    assert_eq!(tailoring(Some("tr-Cyrl")), None);
    assert_eq!(tailoring(Some("az-Latn-AZ")), Some(Tailoring::Turkic));
}
