//! Тело адреса `data:` (RFC 2397): `base64` или процентное кодирование. Чистое
//! преобразование текста адреса в байты, без файловой системы и без смысла картинки.

/// Байты адреса `data:` по его части после схемы (`image/png;base64,…`).
///
/// RFC 2397: без пометки `base64` содержимое лежит прямо в адресе,
/// процентно-кодированным (`%3Csvg…`). Такое встречается у рисунков.
pub(crate) fn decode(rest: &str) -> Option<Vec<u8>> {
    let (head, payload) = rest.split_once(',')?;
    if head.ends_with("base64") {
        return base64_decode(payload);
    }
    Some(percent_decode(payload))
}

/// Процентное кодирование адресов: `%3C` → `<`. Остальные знаки как есть.
pub(crate) fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// Base64 без зависимости: нужен ровно один раз и только на чтение.
pub(crate) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
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
            _ => return None,
        } as u32;
        acc = (acc << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}
