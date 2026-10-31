//! Чистый разбор `initial-letter` до изменения стиля.

pub(super) fn initial_letter(v: &str) -> Option<Option<(f32, u32)>> {
    if v == "normal" {
        return Some(None);
    }
    let (mut size, mut sink) = (None, None);
    for word in v.split_whitespace() {
        match word {
            "drop" if sink.is_none() => sink = Some(0),
            "raise" if sink.is_none() => sink = Some(1),
            w if size.is_none() => {
                size = Some(
                    w.parse::<f32>()
                        .ok()
                        .filter(|n| n.is_finite() && *n >= 1.0)?,
                )
            }
            w if sink.is_none() => sink = Some(w.parse::<u32>().ok().filter(|n| *n >= 1)?),
            _ => return None,
        }
    }
    let size = size?;
    let sink = sink
        .filter(|n| *n != 0)
        .unwrap_or((size.floor() as u32).max(1));
    Some(Some((size, sink)))
}
