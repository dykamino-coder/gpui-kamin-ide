use super::*;

#[test]
fn reads_charge_attempts_and_keep_longer_overlapping_root_copy() {
    let mut copied = Copied(BTreeMap::new());
    let mut coverage = Coverage::default();
    record(
        16,
        32,
        vec![64, 80, 96, 112],
        true,
        &mut copied,
        &mut coverage,
    );
    record(16, 8, vec![64], true, &mut copied, &mut coverage);
    record(32, 64, vec![0; 2], false, &mut copied, &mut coverage);
    assert_eq!(copied.words(16, 32), &[64, 80, 96, 112]);
    assert_eq!(copied.words(16, 8), &[64]);
    assert_eq!(coverage.attempted_bytes, 56);
    assert_eq!(coverage.copied_bytes, 40);
    assert_eq!(coverage.truncated_regions, 1);
    assert_eq!(coverage.unreadable_regions, 1);
}
