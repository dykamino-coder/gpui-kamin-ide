//! Ограниченная история native-диагностики: целые записи, два поколения, без
//! открытого handle между записями (Windows запрещает переименование такого файла).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub(super) struct Log {
    path: PathBuf,
    limit: u64,
}

impl Log {
    pub(super) fn new(path: PathBuf, limit: u64) -> Self {
        Self { path, limit }
    }

    pub(super) fn append(&self, line: &str) -> io::Result<()> {
        let record = format!("{line}\n");
        if record.len() as u64 > self.limit || line.contains(['\n', '\r']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid record",
            ));
        }
        // Старый writer мог оставить очень большой файл. Сохраняем только хвост
        // целых строк; чтение ограничено лимитом, а не размером legacy-файла.
        for path in [&self.path, &self.backup(1), &self.backup(2)] {
            self.trim(path)?;
        }
        let len = length(&self.path)?;
        if len + record.len() as u64 > self.limit {
            remove_if_present(&self.backup(2))?;
            rename_if_present(&self.backup(1), &self.backup(2))?;
            rename_if_present(&self.path, &self.backup(1))?;
        }
        let (mut file, before) = self.open_append()?;
        if let Err(error) = file.write_all(record.as_bytes()) {
            // Не позволяем следующей успешной строке приклеиться к torn record.
            file.set_len(before)?;
            return Err(error);
        }
        Ok(())
    }

    fn open_append(&self) -> io::Result<(File, u64)> {
        // Append-only Windows handle не разрешает set_len для rollback.
        // Sink mutex сериализует один shell writer; seek + write сохраняет
        // append contract и FILE_WRITE_DATA для усечения torn record.
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&self.path)?;
        let before = file.seek(SeekFrom::End(0))?;
        Ok((file, before))
    }

    fn backup(&self, generation: u8) -> PathBuf {
        let mut name = self.path.as_os_str().to_os_string();
        name.push(format!(".{generation}"));
        name.into()
    }

    fn trim(&self, path: &Path) -> io::Result<()> {
        let len = length(path)?;
        if len <= self.limit {
            return Ok(());
        }
        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(len - self.limit - 1))?;
        let mut tail = Vec::new();
        file.take(self.limit + 1).read_to_end(&mut tail)?;
        let start = tail
            .iter()
            .position(|&b| b == b'\n')
            .map_or(tail.len(), |i| i + 1);
        let end = tail
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(start, |i| i + 1);
        // Закрываем read handle до замены, в том числе на Windows. Ошибка записи
        // временного файла оставляет исходную историю нетронутой.
        let temp = path.with_extension(format!(
            "{}.trim",
            path.extension().unwrap_or_default().to_string_lossy()
        ));
        fs::write(&temp, &tail[start..end])?;
        fs::rename(&temp, path)
    }
}

fn length(path: &Path) -> io::Result<u64> {
    match fs::metadata(path) {
        Ok(meta) => Ok(meta.len()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e),
    }
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn rename_if_present(from: &Path, to: &Path) -> io::Result<()> {
    match fs::rename(from, to) {
        Err(e) if e.kind() == io::ErrorKind::NotFound && !from.exists() => Ok(()),
        result => result,
    }
}

#[cfg(test)]
#[path = "diag_log_tests.rs"]
mod tests;
