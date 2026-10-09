//! Реальные COM AddRef/Release: owned getter, early return и смена поколения.

use super::take_owned;
use crate::web::device_generation::Current;
use std::ffi::c_void;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, Interface};

#[repr(C)]
struct Mock {
    vtable: *const IUnknown_Vtbl,
    refs: Arc<AtomicU32>,
}
unsafe extern "system" fn query(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    unsafe {
        if *iid == IUnknown::IID {
            *out = this;
            add(this);
            HRESULT(0)
        } else {
            *out = std::ptr::null_mut();
            HRESULT(0x80004002u32 as i32)
        }
    }
}
unsafe extern "system" fn add(this: *mut c_void) -> u32 {
    unsafe { (&*this.cast::<Mock>()).refs.fetch_add(1, Ordering::SeqCst) + 1 }
}
unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    unsafe {
        let left = (&*this.cast::<Mock>()).refs.fetch_sub(1, Ordering::SeqCst) - 1;
        if left == 0 {
            drop(Box::from_raw(this.cast::<Mock>()));
        }
        left
    }
}
static VTABLE: IUnknown_Vtbl = IUnknown_Vtbl {
    QueryInterface: query,
    AddRef: add,
    Release: release,
};
fn object() -> (IUnknown, Arc<AtomicU32>) {
    let refs = Arc::new(AtomicU32::new(1));
    let raw = Box::into_raw(Box::new(Mock {
        vtable: &VTABLE,
        refs: refs.clone(),
    }));
    (unsafe { IUnknown::from_raw(raw.cast()) }, refs)
}
fn getter(object: &IUnknown) -> IUnknown {
    unsafe { take_owned(Some(object.clone().into_raw())).unwrap() }
}
#[test]
fn same_device_getters_and_early_returns_balance_every_owned_reference() {
    let (device, refs) = object();
    let mut current = Current::default();
    for _ in 0..1000 {
        current.observe(getter(&device), || panic!("same generation cannot retire"));
        assert_eq!(refs.load(Ordering::SeqCst), 2);
        let early_return = || -> Option<()> {
            let _device = getter(&device);
            let _context = unsafe { take_owned::<IUnknown>(None)? };
            Some(())
        };
        assert!(early_return().is_none());
        assert_eq!(refs.load(Ordering::SeqCst), 2);
        // Popup and main frame getters follow the same owned pair contract.
        let pair = (getter(&device), getter(&device));
        drop(pair);
        assert_eq!(refs.load(Ordering::SeqCst), 2);
    }
    drop(current);
    drop(device);
    assert_eq!(refs.load(Ordering::SeqCst), 0);
}
#[test]
fn replacement_retires_before_publish_and_snapshot_keeps_old_device_alive() {
    let (old, old_refs) = object();
    let (new, new_refs) = object();
    let mut current = Current::default();
    current.observe(getter(&old), || panic!("initial generation"));
    let in_flight = current.snapshot().unwrap();
    let mut retired = false;
    current.observe(getter(&new), || {
        retired = true;
        assert_eq!(old_refs.load(Ordering::SeqCst), 3);
    });
    assert!(retired);
    assert!(!current.matches(&in_flight));
    assert!(current.matches(&new));
    assert_eq!(old_refs.load(Ordering::SeqCst), 2);
    drop(old);
    assert_eq!(old_refs.load(Ordering::SeqCst), 1);
    drop(in_flight);
    assert_eq!(old_refs.load(Ordering::SeqCst), 0);
    drop(current);
    drop(new);
    assert_eq!(new_refs.load(Ordering::SeqCst), 0);
}
