//! Interpret valid PDH GPU engine items without mixing adapters or engines.

use super::{Adapter, Field, Utilization};

#[cfg_attr(not(windows), allow(dead_code))]
pub(super) fn select(
    adapter: Adapter,
    items: impl IntoIterator<Item = (String, f64)>,
) -> Field<Utilization> {
    let luid = format!(
        "_luid_0x{:08x}_0x{:08x}_",
        adapter.luid_high as u32, adapter.luid_low
    );
    let mut busiest: Option<Utilization> = None;
    for (engine, percent) in items {
        if !engine.to_ascii_lowercase().contains(&luid) || !percent.is_finite() || percent < 0.0 {
            continue;
        }
        if busiest.as_ref().is_none_or(|value| percent > value.percent) {
            busiest = Some(Utilization {
                engine,
                percent,
                interval_ns: 0,
            });
        }
    }
    busiest
        .map(Field::available)
        .unwrap_or_else(|| Field::unavailable("no valid engine samples for selected adapter"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_selected_luid_and_individual_engine_instead_of_summing_types() {
        let adapter = Adapter {
            luid_high: -1,
            luid_low: 0xab,
        };
        let name = |low, engine| {
            format!("pid_12_luid_0xFFFFFFFF_0x{low:08X}_phys_0_eng_{engine}_engtype_3D")
        };
        let selected = name(0xab, 1);
        let load = select(
            adapter,
            [
                (name(0xab, 0), 30.0),
                (selected.clone(), 40.0),
                (name(0xac, 0), 99.0),
                (name(0xab, 2), f64::NAN),
            ],
        )
        .value
        .unwrap();
        assert_eq!(load.engine, selected);
        assert_eq!(load.percent, 40.0);
        assert!(select(adapter, []).value.is_none());
    }
}
