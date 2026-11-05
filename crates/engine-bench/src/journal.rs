//! Временный журнал измерений: история живёт на диске до сборки итогового JSON.

use serde_json::Value;
use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

// Поле File закрывается раньше Cleanup, в том числе при ошибке записи/чтения.
struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub(super) struct Journal {
    file: File,
    _cleanup: Cleanup,
    count: usize,
}

impl Journal {
    pub(super) fn new() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "kamin-engine-bench-{}-{}.jsonl",
                std::process::id(),
                NEXT.fetch_add(1, Relaxed)
            ));
            match OpenOptions::new()
                .read(true)
                .append(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    return Ok(Self {
                        file,
                        _cleanup: Cleanup(path),
                        count: 0,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub(super) fn push(&mut self, value: &Value) -> io::Result<()> {
        // Буфер и Value освобождаются между кадрами; flush входит только в учёт стенда.
        let mut writer = BufWriter::new(&mut self.file);
        serde_json::to_writer(&mut writer, value)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        self.count += 1;
        Ok(())
    }

    pub(super) fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub(super) fn read(&mut self) -> io::Result<Vec<Value>> {
        self.file.seek(SeekFrom::Start(0))?;
        let records: Vec<Value> =
            serde_json::Deserializer::from_reader(BufReader::new(&mut self.file))
                .into_iter()
                .collect::<Result<_, _>>()
                .map_err(io::Error::other)?;
        if records.len() != self.count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "measurement journal lost records",
            ));
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn repeated_records_survive_replay_and_the_owned_file_is_removed_on_drop() {
        let mut journal = Journal::new().expect("temporary journal");
        let path = journal._cleanup.0.clone();
        let payload = "x".repeat(8192);
        for index in 0..1000 {
            journal
                .push(&json!({"sample": index, "payload": payload, "skipped": null}))
                .unwrap();
        }
        assert!(std::fs::metadata(&path).unwrap().len() > 8_192_000);
        let values = journal.read().unwrap();
        assert_eq!(values.len(), 1000);
        assert_eq!(values[999]["sample"], 999);
        assert_eq!(values[0]["payload"], payload);
        assert!(values[0]["skipped"].is_null());
        journal.push(&json!({"sample": 1000})).unwrap();
        assert_eq!(journal.read().unwrap().len(), 1001);
        drop(journal);
        assert!(!path.exists());
    }

    #[test]
    fn truncated_journal_is_an_error_instead_of_a_shorter_measurement() {
        let mut journal = Journal::new().unwrap();
        journal.push(&json!({"sample": 1})).unwrap();
        OpenOptions::new()
            .write(true)
            .open(&journal._cleanup.0)
            .unwrap()
            .set_len(0)
            .unwrap();
        assert_eq!(
            journal.read().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn replay_preserves_floating_point_bits_as_well_as_integer_counters() {
        let mut journal = Journal::new().unwrap();
        let mut bits = vec![0, 1 << 63, f64::MAX.to_bits(), f64::MIN_POSITIVE.to_bits()];
        let mut seed = 0x3feb_0e70_0000_0001u64;
        for _ in 0..1024 {
            bits.push(seed & 0xffef_ffff_ffff_ffff);
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        }
        let values: Vec<_> = bits.iter().map(|&bits| f64::from_bits(bits)).collect();
        journal
            .push(&json!({"times": values, "counter": 9_007_199_254_740_993u64}))
            .unwrap();
        let records = journal.read().unwrap();
        let actual: Vec<_> = records[0]["times"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap().to_bits())
            .collect();
        assert_eq!(actual, bits);
        assert_eq!(records[0]["counter"].as_u64(), Some(9_007_199_254_740_993));
    }
}
