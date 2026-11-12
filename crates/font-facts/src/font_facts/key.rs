//! Ключ семейства в реестре: имя сравнивается без различия регистра ASCII
//! (css-fonts-4 §2.1: «user agents must match these names case insensitively»),
//! реестр хранит имена в нижнем регистре. Поиск по имени идёт на каждый кусок
//! текста, поэтому ключ строится без кучи: имя уже в нижнем регистре читается
//! как есть, короткое складывается в буфер на стеке, и только длинное имя с
//! заглавными буквами получает собственную строку.

/// Longest family name folded on the stack; real family names are far shorter.
const INLINE: usize = 64;

/// Call `find` with the registry key of `family`.
pub(super) fn with<R>(family: &str, find: impl FnOnce(&str) -> R) -> R {
    let bytes = family.as_bytes();
    if !bytes.iter().any(u8::is_ascii_uppercase) {
        return find(family);
    }
    if bytes.len() <= INLINE {
        let mut folded = [0u8; INLINE];
        let folded = &mut folded[..bytes.len()];
        folded.copy_from_slice(bytes);
        folded.make_ascii_lowercase();
        // Folding ASCII letters keeps every byte of a multi-byte sequence as it was.
        if let Ok(key) = std::str::from_utf8(folded) {
            return find(key);
        }
    }
    find(&family.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{INLINE, with};

    fn key(family: &str) -> String {
        with(family, str::to_owned)
    }

    #[test]
    fn the_key_is_the_name_in_ascii_lower_case() {
        assert_eq!(key("times new roman"), "times new roman");
        assert_eq!(key("Times New Roman"), "times new roman");
        assert_eq!(key("ＭＳ Gothic Ä"), "ＭＳ gothic Ä");
        assert_eq!(key(""), "");
    }

    #[test]
    fn a_name_longer_than_the_stack_buffer_folds_the_same_way() {
        for len in [INLINE - 1, INLINE, INLINE + 1, INLINE * 3] {
            let name = "Aé".repeat(len).chars().take(len).collect::<String>();
            assert_eq!(key(&name), name.to_ascii_lowercase(), "{len}");
        }
    }
}
