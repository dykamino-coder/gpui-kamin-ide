//! Порядок семейств свойств и логических сокращений.

use super::*;

pub(super) fn is_wide(v: &str) -> bool {
    CssWide::parse(v).is_some()
}

/// Логические сокращения радиусов стороны (css-borders-4, tentative).
pub(super) fn is_side_radius(k: &str) -> bool {
    matches!(
        k,
        "border-block-start-radius"
            | "border-block-end-radius"
            | "border-inline-start-radius"
            | "border-inline-end-radius"
    )
}

/// Семья имени внутри блока: НАСТОЯЩЕЕ сокращение со своими длинными
/// свойствами. По одному лишь общему началу судить нельзя: `overflow-wrap` не
/// часть `overflow`, и перестановка этой пары ломает `textarea-pre-wrap-*`.
pub(super) fn family(k: &str, block: Option<&DeclBlock>) -> String {
    let Some(d) = block else {
        return k.into();
    };
    const SHORTHANDS: &[&str] = &["border-image"];
    for root in SHORTHANDS {
        if k.len() > root.len()
            && k.starts_with(root)
            && k.as_bytes().get(root.len()) == Some(&b'-')
            && d.contains(root)
        {
            return (*root).into();
        }
    }
    k.into()
}
