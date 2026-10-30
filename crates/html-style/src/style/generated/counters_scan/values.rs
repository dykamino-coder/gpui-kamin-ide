//! Скан счётчиков: чтение значения имени из директивы `counter-reset`/
//! `counter-increment`/`counter-set` с неявным целым своего свойства
//! (0 у сброса, 1 у приращения) и проверка, названо ли имя, включая
//! обратную запись `reversed(name)`.

/// Value for this name, using the property-specific implicit integer.
pub(super) fn decl_value(decl: &Option<String>, name: &str, default: i64) -> Option<i64> {
    let text = decl.as_deref()?;
    let mut it = text.split_whitespace().peekable();
    let mut found = None;
    while let Some(word) = it.next() {
        let value = match it.peek().and_then(|n| n.parse::<i64>().ok()) {
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
