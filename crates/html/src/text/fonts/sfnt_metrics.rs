//! Sfnt metrics for fonts; split out to keep the owning module within 250 lines.

/// Координата базовой линии `romn` горизонтальной оси таблицы `BASE`
/// (OpenType BASE: Axis → BaseTagList/BaseScriptList → BaseValues →
/// BaseCoord) в долях em; скрипт `latn`, затем `DFLT`, затем первый.
/// `None` — таблицы, оси или тега нет.
pub(super) fn sfnt_romn_baseline(bytes: &[u8]) -> Option<f32> {
    let be16 = |at: usize| -> Option<u16> {
        Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
    };
    let be32 = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes([
            *bytes.get(at)?,
            *bytes.get(at + 1)?,
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]))
    };
    let num_tables = be16(4)? as usize;
    let (mut base, mut head) = (None, None);
    for i in 0..num_tables {
        let rec = 12 + i * 16;
        match bytes.get(rec..rec + 4)? {
            b"BASE" => base = Some(be32(rec + 8)? as usize),
            b"head" => head = Some(be32(rec + 8)? as usize),
            _ => {}
        }
    }
    let (base, head) = (base?, head?);
    let upem = be16(head + 18)? as f32;
    let horiz = be16(base + 4)? as usize;
    if upem <= 0.0 || horiz == 0 {
        return None;
    }
    let axis = base + horiz;
    let (tags_off, scripts_off) = (be16(axis)? as usize, be16(axis + 2)? as usize);
    if tags_off == 0 || scripts_off == 0 {
        return None;
    }
    let (tags, scripts) = (axis + tags_off, axis + scripts_off);
    let tag_count = be16(tags)? as usize;
    let romn = (0..tag_count)
        .find(|i| bytes.get(tags + 2 + i * 4..tags + 6 + i * 4) == Some(b"romn".as_slice()))?;
    let script_count = be16(scripts)? as usize;
    let mut script = None;
    for want in [b"latn", b"DFLT"] {
        for i in 0..script_count {
            let rec = scripts + 2 + i * 6;
            if bytes.get(rec..rec + 4)? == want.as_slice() {
                script = Some(scripts + be16(rec + 4)? as usize);
                break;
            }
        }
        if script.is_some() {
            break;
        }
    }
    let script = match script {
        Some(s) => s,
        None if script_count > 0 => scripts + be16(scripts + 6)? as usize,
        None => return None,
    };
    let values_off = be16(script)? as usize;
    if values_off == 0 {
        return None;
    }
    let values = script + values_off;
    if romn >= be16(values + 2)? as usize {
        return None;
    }
    let coord = values + be16(values + 4 + romn * 2)? as usize;
    Some(be16(coord + 2)? as i16 as f32 / upem)
}

/// Имя семейства из name-таблицы sfnt (nameID 16, затем 1): регистрация
/// в системе может НЕ добавить нового имени в общий список (diff пуст —
/// FontWithFancyFeatures терял алиас и весь текст шёл шрифтом-подменой),
/// поэтому имя читается из самого файла.
pub fn sfnt_family(bytes: &[u8]) -> Option<String> {
    let be16 = |at: usize| -> Option<u16> {
        Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
    };
    let be32 = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes([
            *bytes.get(at)?,
            *bytes.get(at + 1)?,
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]))
    };
    let num_tables = be16(4)? as usize;
    let mut name_off = None;
    for i in 0..num_tables {
        let rec = 12 + i * 16;
        if bytes.get(rec..rec + 4)? == b"name" {
            name_off = Some(be32(rec + 8)? as usize);
            break;
        }
    }
    let base = name_off?;
    let count = be16(base + 2)? as usize;
    let strings = base + be16(base + 4)? as usize;
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let rec = base + 6 + i * 12;
        let platform = be16(rec)?;
        let name_id = be16(rec + 6)?;
        if name_id != 1 && name_id != 16 {
            continue;
        }
        let len = be16(rec + 8)? as usize;
        let off = strings + be16(rec + 10)? as usize;
        let raw = bytes.get(off..off + len)?;
        let text = match platform {
            // Windows/Unicode: UTF-16BE.
            0 | 3 => String::from_utf16(
                &raw.chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            )
            .ok()?,
            _ => String::from_utf8_lossy(raw).into_owned(),
        };
        if text.trim().is_empty() {
            continue;
        }
        // nameID 16 (typographic family) сильнее 1.
        let rank = if name_id == 16 { 2 } else { 1 };
        if best.as_ref().is_none_or(|(r, _)| rank > *r) {
            best = Some((rank, text));
        }
    }
    best.map(|(_, t)| t)
}
