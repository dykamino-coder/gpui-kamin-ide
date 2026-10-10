//! Registered opaque calc handles; no supplied pointer is dereferenced.

#[cfg(feature = "std")]
type Registry = (
    std::vec::Vec<(f32, f32)>,
    std::collections::HashMap<(u32, u32), usize>,
);

#[cfg(feature = "std")]
static PAIRS: std::sync::Mutex<Option<Registry>> = std::sync::Mutex::new(None);

/// Register a pixel term and a fraction of the available percentage basis.
#[cfg(feature = "std")]
pub fn calc_handle(px: f32, fraction: f32) -> *const () {
    let mut guard = PAIRS.lock().unwrap_or_else(|error| error.into_inner());
    let (pairs, index) = guard.get_or_insert_with(Default::default);
    let at = *index
        .entry((px.to_bits(), fraction.to_bits()))
        .or_insert_with(|| {
            pairs.push((px, fraction));
            pairs.len()
        });
    at.checked_mul(8).expect("calc handle registry overflow") as *const ()
}

#[cfg(feature = "std")]
fn pair(handle: *const ()) -> (f32, f32) {
    let address = handle as usize;
    if address == 0 || address & 7 != 0 {
        return (0.0, 0.0);
    }
    let guard = PAIRS.lock().unwrap_or_else(|error| error.into_inner());
    guard
        .as_ref()
        .and_then(|(pairs, _)| pairs.get((address >> 3) - 1).copied())
        .unwrap_or((0.0, 0.0))
}

#[cfg(not(feature = "std"))]
fn pair(_handle: *const ()) -> (f32, f32) {
    (0.0, 0.0)
}

/// Resolve an opaque registered handle without dereferencing it.
pub fn calc_value(handle: *const (), basis: f32) -> f32 {
    let (px, fraction) = pair(handle);
    if fraction == 0.0 {
        px
    } else {
        px + fraction * basis
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn equal_bit_patterns_reuse_handles_and_preserve_both_terms() {
        let handle = calc_handle(12.5, 0.25);
        assert_eq!(handle, calc_handle(12.5, 0.25));
        assert_ne!(handle as usize, 0);
        assert_eq!(handle as usize & 7, 0);
        assert_eq!(calc_value(handle, 200.0), 62.5);
    }

    #[test]
    fn unregistered_and_unaligned_addresses_are_not_dereferenced() {
        assert_eq!(calc_value(core::ptr::null(), 100.0), 0.0);
        assert_eq!(calc_value(3usize as *const (), 100.0), 0.0);
        assert_eq!(calc_value((usize::MAX & !7) as *const (), 100.0), 0.0);
        assert_eq!(calc_value(core::ptr::null(), f32::NAN), 0.0);
    }

    #[test]
    fn pixel_only_calculation_does_not_depend_on_an_unknown_basis() {
        let handle = calc_handle(12.5, 0.0);
        assert_eq!(calc_value(handle, f32::NAN), 12.5);
        assert_eq!(calc_value(handle, f32::INFINITY), 12.5);
    }

    #[test]
    #[cfg(feature = "calc")]
    fn compact_length_tagging_roundtrips_the_opaque_handle() {
        let value = crate::style::Dimension::calc(calc_handle(12.5, 0.25));
        assert_eq!(
            crate::util::MaybeResolve::maybe_resolve(value, Some(100.0), calc_value),
            Some(37.5)
        );
    }
}
