//! Верхние оценки копирования списков nesting, значений и префиксов cascade layers.

use super::Error;

pub(super) fn selectors(head: &str, parent: usize) -> Result<(usize, usize, usize), Error> {
    let amps = head.bytes().filter(|&b| b == b'&').count();
    let width = head.bytes().filter(|&b| b == b',').count() + 1;
    // У каждого элемента списка свой неявный &, явные & также могут копировать
    // весь список. parent уже включает разделители ", "; запас покрывает :is().
    let size = parent
        .checked_add(6)
        .and_then(|n| n.checked_mul(amps.checked_add(width)?))
        .and_then(|n| n.checked_add(head.len().checked_mul(2)?))
        .and_then(|n| n.checked_add(width.checked_mul(3)?))
        .ok_or(Error::CapacityOverflow)?;
    Ok((size, width, amps))
}

pub(super) fn registry(head: &str, parent: usize) -> Result<usize, Error> {
    // Канонический strip_comments оставляет пробел: комментарий не склеивает
    // @layer. Поиск по всей преамбуле консервативен и для recovery/CDO.
    if !head
        .as_bytes()
        .windows(6)
        .any(|s| s.eq_ignore_ascii_case(b"@layer"))
    {
        return Ok(0);
    }
    let segments = head.bytes().filter(|&b| b == b'.' || b == b',').count() + 1;
    // enter_path хранит каждый префикс имени и каждый префикс index path;
    // суммарная длина квадратична по числу сегментов, не по числу блоков.
    parent
        .checked_add(head.len())
        .and_then(|n| n.checked_add(16))
        .and_then(|n| n.checked_mul(segments))
        .ok_or(Error::CapacityOverflow)
}
