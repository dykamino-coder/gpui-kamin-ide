//! Tests compare frames with leaves against frames without them.

#[cfg(test)]
thread_local! {
    static OFF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// No block stands as its leaf on this thread (tests only).
pub(crate) fn off() -> bool {
    #[cfg(test)]
    return OFF.with(|off| off.get());
    #[cfg(not(test))]
    false
}

/// Turn the leaves off or on for this thread's next builds.
#[cfg(test)]
pub(crate) fn set_off(off: bool) {
    OFF.with(|cell| cell.set(off));
}
