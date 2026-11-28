//! Selected-adapter PDH rate query. No counter is opened before explicit opt-in.

use std::time::Instant;

use super::super::{Adapter, Field, Utilization, gpu_load};
use windows::{
    Win32::System::Performance::{
        PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
        PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA, PdhAddEnglishCounterW, PdhCloseQuery,
        PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
    },
    core::{HSTRING, PCWSTR},
};

pub(super) struct Counters {
    adapter: Adapter,
    query: Option<Query>,
    error: Option<String>,
}

struct Query {
    handle: PDH_HQUERY,
    load: PDH_HCOUNTER,
    previous: Option<Instant>,
}

fn check(status: u32) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("PDH status 0x{status:08x}"))
    }
}

impl Counters {
    pub(super) fn new(adapter: Adapter) -> Self {
        match Query::open() {
            Ok(query) => Self {
                adapter,
                query: Some(query),
                error: None,
            },
            Err(error) => Self {
                adapter,
                query: None,
                error: Some(error),
            },
        }
    }

    pub(super) fn poll(&mut self) -> Field<Utilization> {
        let Some(query) = self.query.as_mut() else {
            return Field::unavailable(self.error.as_deref().unwrap_or("PDH query unavailable"));
        };
        // Rate samples require two successful collections, never a fake zero.
        if let Err(error) = check(unsafe { PdhCollectQueryData(query.handle) }) {
            query.previous = None;
            return Field::unavailable(error);
        }
        let now = Instant::now();
        let Some(previous) = query.previous.replace(now) else {
            return Field::unavailable("first GPU sample has no rate interval");
        };
        match values(query.load) {
            Ok(items) => {
                let mut field = gpu_load::select(self.adapter, items);
                if let Some(value) = field.value.as_mut() {
                    value.interval_ns = now
                        .duration_since(previous)
                        .as_nanos()
                        .min(u64::MAX as u128) as u64;
                }
                field
            }
            Err(error) => Field::unavailable(error),
        }
    }
}

impl Query {
    fn open() -> Result<Self, String> {
        let mut handle = PDH_HQUERY::default();
        // The output handle is closed by Query even when adding a counter fails.
        check(unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut handle) })?;
        let mut query = Self {
            handle,
            load: PDH_HCOUNTER::default(),
            previous: None,
        };
        let path = HSTRING::from(format!(
            r"\GPU Engine(pid_{}_*)\Utilization Percentage",
            std::process::id()
        ));
        check(unsafe { PdhAddEnglishCounterW(handle, &path, 0, &mut query.load) })?;
        if check(unsafe { PdhCollectQueryData(handle) }).is_ok() {
            query.previous = Some(Instant::now());
        }
        Ok(query)
    }
}

impl Drop for Query {
    fn drop(&mut self) {
        // This also releases the counters attached to the query.
        let _ = unsafe { PdhCloseQuery(self.handle) };
    }
}

fn values(counter: PDH_HCOUNTER) -> Result<Vec<(String, f64)>, String> {
    let (mut bytes, mut count) = (0u32, 0u32);
    // Probe supplies no storage; the second call can report a growth race.
    let probe = unsafe {
        PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut bytes, &mut count, None)
    };
    if probe != PDH_MORE_DATA {
        check(probe)?;
        return Ok(Vec::new());
    }
    if bytes > 1024 * 1024 {
        return Err("PDH engine array exceeds 1 MiB bound".into());
    }
    let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
    let capacity = buffer.len() * 8;
    let items = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
    check(unsafe {
        PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut bytes, &mut count, Some(items))
    })?;
    if count as usize > capacity / std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>() {
        return Err("PDH returned invalid engine array length".into());
    }
    let mut values = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        // Count was checked against the allocated and correctly aligned buffer.
        let item = unsafe { &*items.add(index) };
        if item.FmtValue.CStatus != PDH_CSTATUS_VALID_DATA
            && item.FmtValue.CStatus != PDH_CSTATUS_NEW_DATA
        {
            continue;
        }
        let start = buffer.as_ptr() as usize;
        let name = item.szName.as_ptr() as usize;
        if name < start || name >= start + capacity || name % 2 != 0 {
            continue;
        }
        // Only the in-buffer portion may be read, even for a missing terminator.
        let units = unsafe {
            std::slice::from_raw_parts(item.szName.as_ptr(), (start + capacity - name) / 2)
        };
        let Some(end) = units.iter().position(|&unit| unit == 0) else {
            continue;
        };
        let Ok(name) = String::from_utf16(&units[..end]) else {
            continue;
        };
        values.push((name, unsafe { item.FmtValue.Anonymous.doubleValue }));
    }
    Ok(values)
}
