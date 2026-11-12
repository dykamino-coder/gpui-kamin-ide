//! Пики памяти на границах и отбрасывание сэмпла, пересёкшегося с границей.

use super::*;
#[test]
fn boundary_samples_preserve_peak_and_distinguish_end_memory() {
    let mut phase = PhaseUsage::default();
    phase.observe(
        Snapshot {
            rss: 100,
            commit: 300,
            ..Default::default()
        },
        Some((80, 40)),
    );
    phase.observe(
        Snapshot {
            rss: 50,
            commit: 400,
            ..Default::default()
        },
        Some((60, 70)),
    );
    assert_eq!((phase.rss, phase.commit), (100, 400));
    assert_eq!((phase.end.rss, phase.end.commit), (50, 400));
    assert_eq!(phase.gpu_peak, Some((80, 70)));
    assert_eq!(phase.gpu_end, Some((60, 70)));
}
#[test]
fn a_sample_read_across_a_boundary_is_dropped() {
    let boundaries = Boundaries(AtomicU64::new(0));
    let epoch = boundaries.quiet().expect("no boundary is reading");
    boundaries.begin();
    assert_eq!(boundaries.quiet(), None, "a boundary is reading");
    assert!(!boundaries.unchanged(epoch));
    boundaries.end();
    assert!(
        !boundaries.unchanged(epoch),
        "the boundary applied a newer reading"
    );
    let later = boundaries.quiet().expect("quiet again");
    assert!(boundaries.unchanged(later));
}
