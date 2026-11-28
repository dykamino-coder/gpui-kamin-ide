//! DbgHelp is process-global and not thread-safe. Each operation, including
//! open/cleanup, holds the same lock; no lock is kept across engine callbacks.

use super::{Module, strip_hash};
use std::{
    mem::{offset_of, size_of},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use windows::{
    Win32::{Foundation::HANDLE, System::Diagnostics::Debug::*},
    core::{HSTRING, PCWSTR},
};

static DBGHELP: Mutex<()> = Mutex::new(());
static NEXT: AtomicUsize = AtomicUsize::new(0x5a0000);

pub(super) struct Symbols {
    handle: HANDLE,
}

impl Symbols {
    pub(super) fn open(modules: &[Module]) -> Result<Self, String> {
        if modules.is_empty() || modules.len() > 256 {
            return Err("symbol module list out of bounds".into());
        }
        let _guard = DBGHELP.lock().unwrap_or_else(|error| error.into_inner());
        let handle = HANDLE(NEXT.fetch_add(1, Ordering::Relaxed) as *mut _);
        let search = modules[0]
            .image
            .parent()
            .ok_or("image has no parent directory")?;
        let search = HSTRING::from(search.as_os_str());
        unsafe { SymInitializeW(handle, &search, false) }.map_err(|error| error.to_string())?;
        let mut result = Ok(());
        for module in modules {
            let image = HSTRING::from(module.image.as_os_str());
            if unsafe {
                SymLoadModuleExW(
                    handle,
                    None,
                    &image,
                    PCWSTR::null(),
                    module.base,
                    0,
                    None,
                    None,
                )
            } == 0
            {
                result = Err(format!(
                    "cannot load symbols for {}",
                    module.image.display()
                ));
                break;
            }
        }
        if let Err(error) = result {
            let _ = unsafe { SymCleanup(handle) };
            return Err(error);
        }
        Ok(Self { handle })
    }

    pub(super) fn frames(&mut self, address: usize) -> Vec<String> {
        let _guard = DBGHELP.lock().unwrap_or_else(|error| error.into_inner());
        let address = address.saturating_sub(1) as u64;
        let mut names = Vec::new();
        // Include inline callers before the physical frame, as the probe's raw
        // return address cannot distinguish an inlined allocation site itself.
        let count = unsafe { SymAddrIncludeInlineTrace(self.handle, address) }.min(128);
        if count > 0 {
            let (mut context, mut frame) = (0, 0);
            if unsafe {
                SymQueryInlineTrace(
                    self.handle,
                    address,
                    0,
                    address,
                    address,
                    &mut context,
                    &mut frame,
                )
            }
            .is_ok()
            {
                for inline in 0..count {
                    if let Some(name) = self.name(address, Some(context + inline)) {
                        names.push(name);
                    }
                }
            }
        }
        names.push(
            self.name(address, None)
                .unwrap_or_else(|| format!("? 0x{address:x}")),
        );
        names
    }

    fn name(&self, address: u64, inline: Option<u32>) -> Option<String> {
        const MAX: usize = 2048;
        // u64 allocation provides SYMBOL_INFOW alignment and room for its
        // flexible UTF-16 name. Only NameLen initialized by DbgHelp is read.
        let bytes = size_of::<SYMBOL_INFOW>() + MAX * 2;
        let mut storage = vec![0u64; bytes.div_ceil(8)];
        let info = storage.as_mut_ptr().cast::<SYMBOL_INFOW>();
        unsafe {
            (*info).SizeOfStruct = size_of::<SYMBOL_INFOW>() as u32;
            (*info).MaxNameLen = MAX as u32;
        }
        let result = unsafe {
            match inline {
                Some(context) => SymFromInlineContextW(self.handle, address, context, None, info),
                None => SymFromAddrW(self.handle, address, None, info),
            }
        };
        if result.is_err() {
            return None;
        }
        let length = unsafe { (*info).NameLen as usize };
        if length >= MAX {
            return None;
        }
        let start = unsafe {
            storage
                .as_ptr()
                .cast::<u8>()
                .add(offset_of!(SYMBOL_INFOW, Name))
                .cast::<u16>()
        };
        let name = String::from_utf16(unsafe { std::slice::from_raw_parts(start, length) }).ok()?;
        Some(strip_hash(name.trim_end_matches('\0').to_owned()))
    }
}

impl Drop for Symbols {
    fn drop(&mut self) {
        let _guard = DBGHELP.lock().unwrap_or_else(|error| error.into_inner());
        let _ = unsafe { SymCleanup(self.handle) };
    }
}
