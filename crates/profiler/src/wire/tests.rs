use super::*;
use kamin_heap::live::{Ledger, Stack};

fn archive() -> Archive {
    let mut ledger = Ledger::new(4, 8);
    ledger.allocate(64, 32, Stack::default());
    ledger.allocate(128, 16, Stack::default());
    ledger.free(128);
    Archive::new(
        Build {
            executable: "fixture.exe".into(),
            revision: "test".into(),
            features: vec![],
            symbols: "fixture".into(),
        },
        vec![],
        Snapshot {
            session: 1,
            epoch: 1,
            sequence: ledger.sequence(),
            elapsed_ns: 0,
            context: Default::default(),
            snapshot_ns: 0,
            history: ledger.history().copied().collect(),
            totals: ledger.totals(),
            profiler_storage_bytes: ledger.storage_bytes(),
            sampling_mean: 0,
            live: ledger.blocks().copied().collect(),
            peak: None,
            estimated_peak: None,
            owners: None,
        },
    )
}

#[test]
fn raw_roundtrip_reconstructs_both_peaks_and_rejects_forged_lifetimes() {
    let mut original = archive();
    let mut bytes = Vec::new();
    original.write(&mut bytes).unwrap();
    let read = Archive::read(&mut bytes.as_slice()).unwrap();
    assert_eq!(read.snapshot.live, original.snapshot.live);
    assert_eq!(read.snapshot.peak.unwrap().len(), 2);
    assert_eq!(
        read.snapshot.estimated_peak.unwrap().amount.observed_bytes,
        48
    );
    original.snapshot.live[0].id = 99;
    assert!(original.write(&mut Vec::new()).is_err());
    original.snapshot.live[0].id = 1;
    original.snapshot.live[0].stack.depth = 33;
    assert!(original.write(&mut Vec::new()).is_err());
}

#[test]
fn dropped_history_remains_unavailable_after_roundtrip() {
    let mut original = archive();
    original.snapshot.history.remove(0);
    original.snapshot.totals.history_dropped = 1;
    let mut bytes = Vec::new();
    original.write(&mut bytes).unwrap();
    let read = Archive::read(&mut bytes.as_slice()).unwrap();
    assert!(read.snapshot.peak.is_none());
    assert!(read.snapshot.estimated_peak.is_none());
    assert!(Archive::read(&mut &bytes[..bytes.len() - 1]).is_err());
}
