//! GPU load and GPU memory of this process from the Windows performance counters
//! (the same source Task Manager shows per process).

#[derive(Clone, Copy, Default)]
pub struct Sample {
    /// Busiest engine type, each type summed over its engines, in percent.
    pub load: Option<f64>,
    pub dedicated: Option<u64>,
    pub shared: Option<u64>,
}

#[cfg(windows)]
pub use imp::Counters;

#[cfg(windows)]
mod imp {
    use super::Sample;
    use std::collections::BTreeMap;
    use windows::{
        Win32::System::Performance::{
            PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
            PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
            PdhGetFormattedCounterArrayW, PdhOpenQueryW,
        },
        core::{HSTRING, PCWSTR},
    };

    pub struct Counters {
        query: PDH_HQUERY,
        load: PDH_HCOUNTER,
        dedicated: PDH_HCOUNTER,
        shared: PDH_HCOUNTER,
    }

    impl Counters {
        pub fn open() -> Option<Self> {
            let pid = std::process::id();
            let mut query = PDH_HQUERY::default();
            // The query handle is written before the call returns and closed in Drop.
            if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != 0 {
                return None;
            }
            let mut this = Self {
                query,
                load: PDH_HCOUNTER::default(),
                dedicated: PDH_HCOUNTER::default(),
                shared: PDH_HCOUNTER::default(),
            };
            let paths = [
                format!(r"\GPU Engine(pid_{pid}_*)\Utilization Percentage"),
                format!(r"\GPU Process Memory(pid_{pid}_*)\Dedicated Usage"),
                format!(r"\GPU Process Memory(pid_{pid}_*)\Shared Usage"),
            ];
            let counters = [&mut this.load, &mut this.dedicated, &mut this.shared];
            for (path, counter) in paths.iter().zip(counters) {
                // The path string outlives the call; the counter belongs to the open query.
                if unsafe { PdhAddEnglishCounterW(query, &HSTRING::from(path), 0, counter) } != 0 {
                    return None;
                }
            }
            // Rate counters need a first collection to form a difference.
            let _ = unsafe { PdhCollectQueryData(query) };
            Some(this)
        }

        pub fn sample(&self) -> Sample {
            // The query stays open for the life of `self`.
            if unsafe { PdhCollectQueryData(self.query) } != 0 {
                return Sample::default();
            }
            let load = values(self.load).map(|engines| {
                let mut by_type = BTreeMap::<String, f64>::new();
                for (name, value) in engines {
                    let kind = name.rsplit("engtype_").next().unwrap_or("").to_owned();
                    *by_type.entry(kind).or_default() += value;
                }
                by_type.into_values().fold(0., f64::max)
            });
            let bytes = |counter| {
                values(counter)
                    .map(|items| items.iter().map(|(_, value)| *value).sum::<f64>() as u64)
            };
            Sample {
                load,
                dedicated: bytes(self.dedicated),
                shared: bytes(self.shared),
            }
        }
    }

    impl Drop for Counters {
        fn drop(&mut self) {
            // Closes the counters added to this query as well.
            let _ = unsafe { PdhCloseQuery(self.query) };
        }
    }

    fn values(counter: PDH_HCOUNTER) -> Option<Vec<(String, f64)>> {
        let (mut bytes, mut count) = (0u32, 0u32);
        // Size probe: no buffer is written.
        let probe = unsafe {
            PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut bytes, &mut count, None)
        };
        if probe != PDH_MORE_DATA {
            return None;
        }
        // u64 storage keeps the item alignment; names live in the same buffer.
        let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
        let items = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        // The buffer holds `bytes` bytes, as the probe asked.
        let read = unsafe {
            PdhGetFormattedCounterArrayW(
                counter,
                PDH_FMT_DOUBLE,
                &mut bytes,
                &mut count,
                Some(items),
            )
        };
        if read != 0 {
            return None;
        }
        Some(
            (0..count as usize)
                .map(|index| {
                    // `count` items were written; each name points into `buffer`.
                    let item = unsafe { &*items.add(index) };
                    let name = unsafe { item.szName.to_string() }.unwrap_or_default();
                    (name, unsafe { item.FmtValue.Anonymous.doubleValue })
                })
                .collect(),
        )
    }
}

#[cfg(not(windows))]
pub struct Counters;

#[cfg(not(windows))]
impl Counters {
    pub fn open() -> Option<Self> {
        None
    }

    pub fn sample(&self) -> Sample {
        Sample::default()
    }
}
