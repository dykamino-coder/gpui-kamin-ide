//! KaminIDE patch: registered opaque calc handles owned by a `TaffyTree`; no
//! supplied pointer is dereferenced.
//!
//! A handle is the 1-based index of a `(px, fraction)` pair times 8 (the low
//! three bits of a `CompactLength` calc pointer are tag bits). Upstream taffy
//! leaves the calc value to the tree (`LayoutPartialTree::resolve_calc_value`),
//! so the registry belongs to the tree: `TaffyTree::clear` drops it with the
//! nodes whose styles carry the handles. The host registers pairs inside
//! `TaffyTree::calc_scope` (while converting styles) and the tree resolves
//! them inside `compute_layout_with_measure`; leaf measure functions, which
//! cannot see the tree, read the same scope through `calc_value`.

#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};

/// The `(px, fraction)` pairs of one tree, deduplicated by bit pattern.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub(crate) struct CalcRegistry {
    pairs: std::vec::Vec<(f32, f32)>,
    index: std::collections::HashMap<(u32, u32), usize>,
}

#[cfg(feature = "std")]
impl CalcRegistry {
    fn handle(&mut self, px: f32, fraction: f32) -> *const () {
        let pairs = &mut self.pairs;
        let at = *self
            .index
            .entry((px.to_bits(), fraction.to_bits()))
            .or_insert_with(|| {
                pairs.push((px, fraction));
                pairs.len()
            });
        at.checked_mul(8).expect("calc handle registry overflow") as *const ()
    }

    fn pair(&self, handle: *const ()) -> Option<(f32, f32)> {
        let address = handle as usize;
        if address == 0 || address & 7 != 0 {
            return None;
        }
        self.pairs.get((address >> 3) - 1).copied()
    }

    pub(crate) fn clear(&mut self) {
        self.pairs.clear();
        self.index.clear();
    }
}

/// Shared handle to a tree's registry (the tree stays `Send`).
#[cfg(feature = "std")]
pub(crate) type SharedRegistry = Arc<Mutex<CalcRegistry>>;

#[cfg(feature = "std")]
std::thread_local! {
    static CURRENT: std::cell::RefCell<std::vec::Vec<SharedRegistry>> =
        const { std::cell::RefCell::new(std::vec::Vec::new()) };
}

/// Guard of `TaffyTree::calc_scope`: the tree's registry is current until drop.
#[cfg(feature = "std")]
pub struct CalcScope {
    depth: usize,
    _not_send: core::marker::PhantomData<*const ()>,
}

#[cfg(feature = "std")]
pub(crate) fn enter(registry: &SharedRegistry) -> CalcScope {
    let depth = CURRENT.with(|c| {
        let mut c = c.borrow_mut();
        c.push(registry.clone());
        c.len()
    });
    CalcScope {
        depth,
        _not_send: core::marker::PhantomData,
    }
}

#[cfg(feature = "std")]
impl Drop for CalcScope {
    fn drop(&mut self) {
        let _ = CURRENT.try_with(|c| c.borrow_mut().truncate(self.depth - 1));
    }
}

#[cfg(feature = "std")]
fn with_current<R>(f: impl FnOnce(&mut CalcRegistry) -> R) -> Option<R> {
    let registry = CURRENT.with(|c| c.borrow().last().cloned())?;
    let mut guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    Some(f(&mut guard))
}

/// Register a pixel term and a fraction of the available percentage basis in
/// the current tree's registry (`TaffyTree::calc_scope`). Outside a scope
/// there is no owner: a debug build stops, a release build gets the null
/// handle, which resolves to 0.
#[cfg(feature = "std")]
pub fn calc_handle(px: f32, fraction: f32) -> *const () {
    let handle = with_current(|r| r.handle(px, fraction));
    debug_assert!(
        handle.is_some(),
        "calc handle registered outside TaffyTree::calc_scope"
    );
    handle.unwrap_or(core::ptr::null())
}

#[cfg(feature = "std")]
fn pair(handle: *const ()) -> (f32, f32) {
    let found = with_current(|r| r.pair(handle)).flatten();
    debug_assert!(
        found.is_some() || handle.is_null(),
        "calc handle unknown to the current tree: {handle:?}"
    );
    found.unwrap_or((0.0, 0.0))
}

#[cfg(not(feature = "std"))]
fn pair(_handle: *const ()) -> (f32, f32) {
    (0.0, 0.0)
}

/// Resolve an opaque registered handle without dereferencing it. An unknown
/// handle is a broken link (debug assertion); it resolves to 0 explicitly.
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

    fn registry() -> SharedRegistry {
        SharedRegistry::default()
    }

    #[test]
    fn equal_bit_patterns_reuse_handles_and_preserve_both_terms() {
        let tree = registry();
        let _scope = enter(&tree);
        let handle = calc_handle(12.5, 0.25);
        assert_eq!(handle, calc_handle(12.5, 0.25));
        assert_ne!(handle as usize, 0);
        assert_eq!(handle as usize & 7, 0);
        assert_eq!(calc_value(handle, 200.0), 62.5);
    }

    #[test]
    fn null_handle_resolves_to_zero() {
        let tree = registry();
        let _scope = enter(&tree);
        assert_eq!(calc_value(core::ptr::null(), 100.0), 0.0);
        assert_eq!(calc_value(core::ptr::null(), f32::NAN), 0.0);
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic(expected = "unknown to the current tree")
    )]
    fn handles_of_another_tree_are_not_resolved() {
        let (a, b) = (registry(), registry());
        let handle = {
            let _scope = enter(&a);
            calc_handle(12.5, 0.25)
        };
        let _scope = enter(&b);
        assert_eq!(calc_value(handle, 100.0), 0.0);
    }

    #[test]
    fn pixel_only_calculation_does_not_depend_on_an_unknown_basis() {
        let tree = registry();
        let _scope = enter(&tree);
        let handle = calc_handle(12.5, 0.0);
        assert_eq!(calc_value(handle, f32::NAN), 12.5);
        assert_eq!(calc_value(handle, f32::INFINITY), 12.5);
    }

    #[test]
    fn clearing_the_tree_frees_its_pairs() {
        let tree = registry();
        let _scope = enter(&tree);
        calc_handle(1.0, 0.5);
        tree.lock().unwrap().clear();
        assert!(tree.lock().unwrap().pairs.is_empty());
    }

    #[test]
    #[cfg(feature = "calc")]
    fn compact_length_tagging_roundtrips_the_opaque_handle() {
        let tree = registry();
        let _scope = enter(&tree);
        let value = crate::style::Dimension::calc(calc_handle(12.5, 0.25));
        assert_eq!(
            crate::util::MaybeResolve::maybe_resolve(value, Some(100.0), calc_value),
            Some(37.5)
        );
    }
}
