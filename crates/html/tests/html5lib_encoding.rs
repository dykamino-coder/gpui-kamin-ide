//! Прогон части html5lib-tests про кодировки.
//!
//! Формат: блок `#data` с байтами документа и `#encoding` с меткой, которую
//! обязан определить предпросмотр. Наш предпросмотр — `kamin_html::encoding`.

use std::path::{Path, PathBuf};

fn suite_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/html5lib-tests/encoding")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("vendor/html5lib-tests/encoding"))
}

/// Разбор `.dat`: блоки идут до строки `#encoding`, метка — следом.
fn cases(text: &str) -> Vec<(Vec<u8>, String)> {
    let mut out = vec![];
    let mut data: Vec<u8> = vec![];
    let mut in_data = false;
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        match line {
            "#data" => {
                in_data = true;
                data.clear();
            }
            "#encoding" => {
                in_data = false;
                if let Some(label) = lines.next() {
                    // Последний перевод строки принадлежит разделителю блоков.
                    if data.last() == Some(&b'\n') {
                        data.pop();
                    }
                    out.push((std::mem::take(&mut data), label.trim().to_string()));
                }
            }
            _ if in_data => {
                data.extend_from_slice(line.as_bytes());
                data.push(b'\n');
            }
            _ => {}
        }
    }
    out
}

#[test]
fn encoding_detection() {
    let dir = suite_dir();
    if !dir.exists() {
        panic!("нет набора html5lib-tests: {}", dir.display());
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "dat"))
        .collect();
    files.sort();

    let (mut passed, mut failures) = (0usize, Vec::<String>::new());
    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        let raw = std::fs::read(file).unwrap();
        let text = String::from_utf8_lossy(&raw).to_string();
        for (bytes, want) in cases(&text) {
            let got = kamin_html::encoding::sniff(&bytes);
            // Метки сравниваются по имени кодировки: набор пишет их в разном
            // регистре и разными псевдонимами одной и той же кодировки.
            let expected = encoding_rs::Encoding::for_label(want.as_bytes());
            let same = expected.is_some_and(|e| e == got);
            if same {
                passed += 1;
            } else {
                failures.push(format!(
                    "{name}: ждали {want}, получили {}\n  вход: {:?}",
                    got.name(),
                    String::from_utf8_lossy(&bytes[..bytes.len().min(200)]),
                ));
            }
        }
    }
    if !failures.is_empty() {
        let report =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/html5lib-encoding.txt");
        let _ = std::fs::write(&report, failures.join("\n\n"));
        panic!(
            "кодировка определена иначе: {} падений из {}; разбор: {}",
            failures.len(),
            failures.len() + passed,
            report.display(),
        );
    }
    println!("encoding: {passed} прогонов зелёные");
}
