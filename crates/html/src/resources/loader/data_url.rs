//! Тело адреса `data:` (RFC 2397): `base64` или процентное кодирование. Чистое
//! преобразование текста адреса в байты, без файловой системы и без смысла картинки.
use super::{LoadError, limits};

/// Байты адреса `data:` по его части после схемы (`image/png;base64,…`).
///
/// RFC 2397: без пометки `base64` содержимое лежит прямо в адресе,
/// процентно-кодированным (`%3Csvg…`). Такое встречается у рисунков.
pub(crate) fn decode(rest: &str) -> Result<Vec<u8>, LoadError> {
    decode_bounded(rest, limits::DATA_URL_BYTES, limits::RESOURCE_BYTES)
}

fn decode_bounded(rest: &str, input_limit: usize, limit: usize) -> Result<Vec<u8>, LoadError> {
    // Проверяем весь закодированный вход, включая заголовок и пробелы base64.
    limits::check(rest.len(), input_limit)?;
    let (head, payload) = rest.split_once(',').ok_or(LoadError::InvalidUrl)?;
    if head.ends_with("base64") {
        return base64_bounded(payload, limit);
    }
    let count = percent_bytes(payload).take(limit.saturating_add(1)).count();
    limits::check(count, limit)?;
    let mut out = buffer(count)?;
    out.extend(percent_bytes(payload));
    Ok(out)
}

/// Процентное кодирование адресов: `%3C` → `<`. Остальные знаки как есть.
pub(crate) fn percent_decode(text: &str) -> Vec<u8> {
    percent_bytes(text).collect()
}

fn percent_bytes(text: &str) -> impl Iterator<Item = u8> + '_ {
    let bytes = text.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        let byte = *bytes.get(i)?;
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
        {
            i += 3;
            Some(v)
        } else {
            i += 1;
            Some(byte)
        }
    })
}

/// Base64 без зависимости: нужен ровно один раз и только на чтение.
#[cfg(test)]
pub(crate) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    base64_bounded(text, limits::RESOURCE_BYTES).ok()
}

fn buffer(count: usize) -> Result<Vec<u8>, LoadError> {
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| LoadError::Io(std::io::ErrorKind::OutOfMemory))?;
    Ok(out)
}

fn base64_bounded(text: &str, limit: usize) -> Result<Vec<u8>, LoadError> {
    // Первый проход проверяет бюджет и синтаксис до выделения выходного буфера.
    let mut count = 0usize;
    visit_base64(text, |_| {
        count += 1;
        limits::check(count, limit)
    })?;
    let mut out = buffer(count)?;
    visit_base64(text, |byte| {
        out.push(byte);
        Ok(())
    })?;
    Ok(out)
}

fn visit_base64(
    text: &str,
    mut emit: impl FnMut(u8) -> Result<(), LoadError>,
) -> Result<(), LoadError> {
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for ch in text.bytes() {
        let val = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\n' | b'\r' | b' ' | b'\t' => continue,
            _ => return Err(LoadError::InvalidUrl),
        } as u32;
        acc = (acc << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            emit((acc >> bits) as u8)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_budget_counts_bytes_after_percent_decoding() {
        for input in [",abcd", ",%61%62%63%64", ",a%62c%64"] {
            assert_eq!(decode_bounded(input, 64, 4).unwrap(), b"abcd");
            assert_eq!(
                decode_bounded(input, 64, 3),
                Err(LoadError::TooLarge { limit: 3 })
            );
        }
        assert_eq!(decode_bounded(",%E2%82%AC", 64, 3).unwrap(), "€".as_bytes());
        assert_eq!(decode_bounded(",%broken", 64, 8).unwrap(), b"%broken");
    }

    #[test]
    fn base64_budget_counts_output_and_preserves_padding_and_whitespace() {
        for input in [";base64,YWJjZA==", ";base64,Y W\tJj\r\nZA=="] {
            assert_eq!(decode_bounded(input, 64, 4).unwrap(), b"abcd");
            assert_eq!(
                decode_bounded(input, 64, 3),
                Err(LoadError::TooLarge { limit: 3 })
            );
        }
        assert_eq!(
            decode_bounded(";base64,?", 64, 4),
            Err(LoadError::InvalidUrl)
        );
    }

    #[test]
    fn encoded_budget_includes_header_and_ignored_base64_characters() {
        for input in ["text/plain,", ";base64,     ", ",%61%62"] {
            assert!(decode_bounded(input, input.len(), 32).is_ok());
            assert_eq!(
                decode_bounded(input, input.len() - 1, 32),
                Err(LoadError::TooLarge {
                    limit: input.len() - 1
                })
            );
        }
        assert_eq!(decode_bounded(",", 1, 0).unwrap(), b"");
        assert_eq!(
            decode_bounded(",x", 2, 0),
            Err(LoadError::TooLarge { limit: 0 })
        );
        assert_eq!(
            decode_bounded("no comma", 64, 32),
            Err(LoadError::InvalidUrl)
        );
    }
}
