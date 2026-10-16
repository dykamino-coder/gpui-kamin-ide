//! Used color schemes and the preference passed to embedded documents.

use std::cell::Cell;

#[derive(Clone, Copy, Default)]
struct Scheme {
    dark: bool,
    explicit: bool,
    preferred: bool,
}

thread_local! {
    static SCHEME: Cell<Scheme> = const { Cell::new(Scheme {
        dark: false, explicit: false, preferred: false,
    }) };
}

pub fn dark_scheme() -> bool {
    SCHEME.with(|s| s.get().dark)
}

pub fn set_dark_scheme(dark: bool) -> bool {
    SCHEME.with(|s| {
        let mut value = s.get();
        let old = value.dark;
        value.dark = dark;
        s.set(value);
        old
    })
}

pub(crate) fn reset_scheme(preferred: bool) {
    SCHEME.with(|s| {
        s.set(Scheme {
            preferred,
            ..Scheme::default()
        })
    });
}

pub(crate) fn scheme_state() -> (bool, bool) {
    SCHEME.with(|s| {
        let v = s.get();
        (v.dark, v.explicit)
    })
}

pub(crate) fn restore_scheme((dark, explicit): (bool, bool)) {
    SCHEME.with(|s| {
        let mut v = s.get();
        v.dark = dark;
        v.explicit = explicit;
        s.set(v);
    });
}

pub(crate) fn embedding_dark_scheme() -> bool {
    SCHEME.with(|s| {
        let v = s.get();
        if v.explicit { v.dark } else { v.preferred }
    })
}

pub(crate) fn resolve_scheme(value: &str) {
    let value = value.to_ascii_lowercase();
    let words: Vec<_> = value.split_whitespace().collect();
    if words.iter().any(|w| matches!(*w, "inherit" | "unset")) {
        return;
    }
    SCHEME.with(|s| {
        let mut v = s.get();
        v.explicit = !words.iter().any(|w| matches!(*w, "normal" | "initial"));
        let light = words.contains(&"light");
        let dark = words.contains(&"dark");
        v.dark = dark && (!light || v.preferred);
        s.set(v);
    });
}
