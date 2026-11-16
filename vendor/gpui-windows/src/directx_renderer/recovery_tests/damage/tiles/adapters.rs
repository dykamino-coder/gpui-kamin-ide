//! KaminIDE patch: the adapters the tile pass tests run on.

use super::*;

/// Every adapter the tests can reach: the default one (hardware when the
/// machine has it), the high-performance one if it differs, and WARP.
pub(super) fn devices() -> Vec<(String, DirectXDevices)> {
    use crate::GpuAdapterPreference as P;
    let mut out: Vec<(String, DirectXDevices)> = Vec::new();
    for p in [P::Default, P::HighPerformance, P::Software] {
        let Ok(d) = DirectXDevices::with_instrumentation(p, gpui::Instrumentation::default()) else {
            continue;
        };
        let desc = unsafe { d.adapter.GetDesc1() }.unwrap();
        let name = String::from_utf16_lossy(&desc.Description).trim_matches('\0').to_string();
        if out.iter().all(|(n, _)| *n != name) {
            out.push((name, d));
        }
    }
    assert!(out.len() >= 2, "a hardware adapter and WARP");
    out
}
