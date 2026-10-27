//! Бюджеты входных байтов загрузчика; метаданные не заменяют ограничение чтения.
use super::LoadError;
use std::io::Read;

// Это предел ресурса до декодирования изображения/шрифта, не бюджет raster-кэша.
// Большие шрифты и изображения не должны упираться в лимит внешнего use (16 МиБ).
pub(super) const RESOURCE_BYTES: usize = crate::budget::DocumentLimits::RESOURCE_BYTES as usize;
// Процентное кодирование расширяет каждый байт втрое; заголовок учитывается отдельно.
pub(super) const DATA_URL_BYTES: usize = crate::budget::DocumentLimits::DATA_URL_BYTES as usize;

/// `admit` допускает каждую порцию в сумму документа до роста буфера; контрольный
/// байт сверх предела ресурса не копируется и не списывается.
pub(super) fn read(
    mut reader: impl Read,
    limit: usize,
    mut admit: impl FnMut(usize) -> Result<(), LoadError>,
) -> Result<Vec<u8>, LoadError> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    while bytes.len() < limit {
        let available = chunk.len().min(limit - bytes.len());
        let count = read_chunk(&mut reader, &mut chunk[..available])?;
        if count == 0 {
            return Ok(bytes);
        }
        admit(count)?;
        if bytes.capacity() - bytes.len() < count {
            let capacity = bytes
                .capacity()
                .saturating_mul(2)
                .max(bytes.len() + count)
                .min(limit);
            bytes
                .try_reserve_exact(capacity - bytes.len())
                .map_err(|_| LoadError::Io(std::io::ErrorKind::OutOfMemory))?;
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    // Один байт сверх бюджета нужен только для различения EOF и превышения.
    if read_chunk(&mut reader, &mut chunk[..1])? == 0 {
        Ok(bytes)
    } else {
        Err(LoadError::TooLarge { limit })
    }
}

fn read_chunk(reader: &mut impl Read, bytes: &mut [u8]) -> Result<usize, LoadError> {
    loop {
        match reader.read(bytes) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => return result.map_err(io_error),
        }
    }
}

pub(super) fn check(len: usize, limit: usize) -> Result<(), LoadError> {
    if len > limit {
        Err(LoadError::TooLarge { limit })
    } else {
        Ok(())
    }
}

pub(super) fn io_error(error: std::io::Error) -> LoadError {
    match error.kind() {
        std::io::ErrorKind::NotFound => LoadError::NotFound,
        kind => LoadError::Io(kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{BudgetError, BudgetKind, DocumentBudget, DocumentLimits};
    use crate::resources::loader::{Cancellation, ResourceUrl, SyncLoader, fetch};

    fn free(_: usize) -> Result<(), LoadError> {
        Ok(())
    }

    fn load(url: &ResourceUrl) -> Result<crate::resources::loader::Loaded, LoadError> {
        let revision = crate::budget::RevisionBudget::standalone();
        fetch(&SyncLoader, url, &revision, &Cancellation::default())
    }

    #[test]
    fn file_chunks_are_admitted_before_the_buffer_grows() {
        let path = std::env::temp_dir().join(format!("kamin-ingress-{}.bin", std::process::id()));
        std::fs::write(&path, vec![7; 20_000]).unwrap();
        let (url, _) = ResourceUrl::resolve(path.to_str().unwrap(), None);
        for (limit, ok) in [(19_999, false), (20_000, true), (20_001, true)] {
            let limits = DocumentLimits::default()
                .lower(BudgetKind::ResourceBytes, limit)
                .unwrap();
            let governor = crate::budget::ResourceGovernor::default();
            let revision = DocumentBudget::new(limits, governor).unwrap().revision();
            let result = fetch(&SyncLoader, &url, &revision, &Cancellation::default());
            assert_eq!(result.is_ok(), ok);
            // Отказ на третьей порции: списаны только две допущенные порции по 8 КиБ.
            let used = if ok { 20_000 } else { 16_384 };
            assert_eq!(revision.used(BudgetKind::ResourceBytes), used);
            if !ok {
                assert!(matches!(
                    result.unwrap_err(),
                    LoadError::Budget(BudgetError::Exceeded(e)) if e.increment == 3_616
                ));
            }
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn growing_or_unending_input_stops_after_one_probe_byte() {
        struct Counting {
            count: usize,
        }
        impl Read for Counting {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                self.count += out.len();
                out.fill(1);
                Ok(out.len())
            }
        }
        let mut reader = Counting { count: 0 };
        assert_eq!(
            read(&mut reader, 17, free),
            Err(LoadError::TooLarge { limit: 17 })
        );
        assert_eq!(reader.count, 18);
        assert_eq!(read(std::io::empty(), 0, free).unwrap(), b"");
        assert_eq!(
            read(std::io::repeat(0), 0, free),
            Err(LoadError::TooLarge { limit: 0 })
        );
    }

    #[test]
    fn boundary_is_inclusive_and_io_errors_survive() {
        assert_eq!(read(&b"1234"[..], 4, free).unwrap(), b"1234");
        assert_eq!(read(&b"123"[..], 4, free).unwrap(), b"123");
        struct Failure;
        impl Read for Failure {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::PermissionDenied.into())
            }
        }
        assert_eq!(
            read(Failure, 4, free),
            Err(LoadError::Io(std::io::ErrorKind::PermissionDenied))
        );
    }

    #[test]
    fn sync_file_loader_accepts_large_resources_and_rejects_oversized_metadata() {
        let path =
            std::env::temp_dir().join(format!("kamin-loader-budget-{}.bin", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(17 * 1024 * 1024).unwrap();
        let (url, _) = ResourceUrl::resolve(path.to_str().unwrap(), None);
        assert_eq!(load(&url).unwrap().bytes.len(), 17 * 1024 * 1024);
        file.set_len(RESOURCE_BYTES as u64 + 1).unwrap();
        assert_eq!(
            load(&url).unwrap_err(),
            LoadError::TooLarge {
                limit: RESOURCE_BYTES
            }
        );
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
}
