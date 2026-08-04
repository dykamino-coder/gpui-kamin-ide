//! Имя codicon → глиф (ThemeIcon-вклады VSIX, `$(icon)` в подписях).
//!
//! Таблица — ДАННЫЕ: `assets/icons/codicons.txt`, по строке «имя<TAB>код»,
//! где код — шестнадцатеричный номер символа в шрифте codicon. Сгенерирована
//! из `codicon.css` того же пакета `@vscode/codicons`, что шипит оригинал;
//! в виде `.rs` она занимала 661 строку и правилась только генератором.

use std::collections::HashMap;
use std::sync::LazyLock;

static MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    include_str!("../../assets/icons/codicons.txt")
        .lines()
        .filter_map(|line| {
            let (name, code) = line.split_once('\t')?;
            let ch = char::from_u32(u32::from_str_radix(code, 16).ok()?)?;
            // Глиф живёт до конца работы программы — строка утекает один раз
            Some((name, &*Box::leak(ch.to_string().into_boxed_str())))
        })
        .collect()
});

/// Глиф по имени codicon. `None` — такого имени в наборе нет.
pub fn codicon_by_name(name: &str) -> Option<&'static str> {
    MAP.get(name).copied()
}

#[cfg(test)]
mod tests {
    use super::codicon_by_name;

    #[test]
    fn known_and_unknown_names() {
        assert_eq!(codicon_by_name("account"), Some("\u{eb99}"));
        assert_eq!(codicon_by_name("zoom-out"), Some("\u{eb82}"));
        assert_eq!(codicon_by_name("нет-такой-иконки"), None);
    }
}
