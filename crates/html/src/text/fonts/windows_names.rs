//! Windows names for fonts; split out to keep the owning module within 250 lines.

/// Дописать в name-таблицу записи Windows (platform 3, en-US).
///
/// DirectWrite строит имена семейства только из записей platform 3 (или Mac);
/// шрифт, где имена лежат лишь под platform 0 / language 0 (обычное дело для
/// тестовых шрифтов WPT — FontWithFancyFeatures и родня), получает ПУСТОЙ
/// набор локализованных имён: его не найти по имени, и текст молча рисуется
/// системной подменой. Строки platform 0 — тот же UTF-16BE, так что записи
/// просто дублируются с (3, 1, 0x0409) поверх того же строкового блоба;
/// пересобранная таблица дописывается в конец файла, а запись каталога
/// перенацеливается на неё.
pub(super) fn ensure_windows_names(bytes: Vec<u8>) -> Vec<u8> {
    fn be16(b: &[u8], at: usize) -> Option<u16> {
        Some(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?]))
    }
    fn be32(b: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_be_bytes([
            *b.get(at)?,
            *b.get(at + 1)?,
            *b.get(at + 2)?,
            *b.get(at + 3)?,
        ]))
    }
    let rebuild = || -> Option<Vec<u8>> {
        let num_tables = be16(&bytes, 4)? as usize;
        let mut dir_at = None;
        for i in 0..num_tables {
            let rec = 12 + i * 16;
            if bytes.get(rec..rec + 4)? == b"name" {
                dir_at = Some(rec);
                break;
            }
        }
        let rec = dir_at?;
        let base = be32(&bytes, rec + 8)? as usize;
        let count = be16(&bytes, base + 2)? as usize;
        let str_off = base + be16(&bytes, base + 4)? as usize;
        let mut records: Vec<[u16; 6]> = Vec::with_capacity(count * 2);
        let mut str_end = 0usize;
        for i in 0..count {
            let r = base + 6 + i * 12;
            let rec6 = [
                be16(&bytes, r)?,
                be16(&bytes, r + 2)?,
                be16(&bytes, r + 4)?,
                be16(&bytes, r + 6)?,
                be16(&bytes, r + 8)?,
                be16(&bytes, r + 10)?,
            ];
            if rec6[0] == 3 && rec6[3] == 1 {
                // Имя семейства для Windows уже есть — файл не трогается.
                return None;
            }
            str_end = str_end.max(rec6[4] as usize + rec6[5] as usize);
            records.push(rec6);
        }
        let blob = bytes.get(str_off..str_off + str_end)?.to_vec();
        let mut extra: Vec<[u16; 6]> = records
            .iter()
            .filter(|r| r[0] == 0 && matches!(r[3], 1 | 2 | 4 | 6 | 16 | 17))
            .map(|r| [3, 1, 0x0409, r[3], r[4], r[5]])
            .collect();
        if extra.is_empty() {
            return None;
        }
        records.append(&mut extra);
        records.sort();
        // Новая таблица целиком, в конец файла.
        let mut table = Vec::with_capacity(6 + records.len() * 12 + blob.len());
        for v in [0u16, records.len() as u16, 6 + records.len() as u16 * 12] {
            table.extend_from_slice(&v.to_be_bytes());
        }
        for r in &records {
            for v in r {
                table.extend_from_slice(&v.to_be_bytes());
            }
        }
        table.extend_from_slice(&blob);
        let mut out = bytes.clone();
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        let new_off = out.len() as u32;
        out.extend_from_slice(&table);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        // Каталог: offset, length и честная контрольная сумма таблицы.
        let sum = table
            .chunks(4)
            .map(|c| {
                let mut w = [0u8; 4];
                w[..c.len()].copy_from_slice(c);
                u32::from_be_bytes(w)
            })
            .fold(0u32, u32::wrapping_add);
        out[rec + 4..rec + 8].copy_from_slice(&sum.to_be_bytes());
        out[rec + 8..rec + 12].copy_from_slice(&new_off.to_be_bytes());
        out[rec + 12..rec + 16].copy_from_slice(&(table.len() as u32).to_be_bytes());
        Some(out)
    };
    rebuild().unwrap_or(bytes)
}
