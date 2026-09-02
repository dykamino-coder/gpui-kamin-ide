//! Ротация crash.log между запусками приложения.
//!
//! Только главный процесс сдвигает набор: CEF-дети стартуют параллельно и не
//! должны переименовывать общий файл друг у друга. Во время одного запуска все
//! процессы лишь append'ят в новый current log.

use std::path::Path;

pub(crate) fn rotate(path: &Path, backups: usize) {
    if backups == 0 {
        let _ = std::fs::remove_file(path);
        return;
    }
    let backup = |n: usize| {
        path.with_file_name(format!(
            "{}.{}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("crash.log"),
            n
        ))
    };
    let _ = std::fs::remove_file(backup(backups));
    for i in (1..backups).rev() {
        let from = backup(i);
        if from.exists() {
            let _ = std::fs::rename(from, backup(i + 1));
        }
    }
    if path.exists() {
        let _ = std::fs::rename(path, backup(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_a_bounded_number_of_prior_runs() {
        let dir = std::env::temp_dir().join(format!(
            "kamin-crash-rotate-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("crash.log");
        std::fs::write(&path, "current").unwrap();
        std::fs::write(dir.join("crash.log.1"), "previous").unwrap();
        std::fs::write(dir.join("crash.log.2"), "oldest").unwrap();

        rotate(&path, 2);

        assert_eq!(
            std::fs::read_to_string(dir.join("crash.log.1")).unwrap(),
            "current"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("crash.log.2")).unwrap(),
            "previous"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
