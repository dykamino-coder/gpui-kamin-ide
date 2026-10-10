//! Read counter directive values with the property-specific default integer.

/// Value for this name, using the property-specific implicit integer.
pub(super) fn decl_value(decl: &Option<String>, name: &str, default: i32) -> Option<i32> {
    let text = decl.as_deref()?;
    let mut it = text.split_whitespace().peekable();
    let mut found = None;
    while let Some(word) = it.next() {
        let value = match it.peek().and_then(|n| n.parse::<i32>().ok()) {
            Some(v) => {
                it.next();
                v
            }
            None => default,
        };
        if word == name {
            found = Some(value);
        }
    }
    found
}

/// Названо ли имя в объявлении сброса — в том числе обратной записью.
pub(super) fn decl_has(decl: &Option<String>, name: &str) -> bool {
    let reversed = format!("reversed({name})");
    decl.as_deref().is_some_and(|t| {
        t.split_whitespace()
            .any(|w| w == name || w == reversed.as_str())
    })
}
