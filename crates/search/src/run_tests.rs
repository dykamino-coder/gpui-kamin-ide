use super::*;
use crate::args::parse;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn tree(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("kamin-search-run-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for i in 0..40 {
        let dir = root.join(format!("d{}", i % 4));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("f{i}.txt")), format!("hit {i}\nmiss\n")).unwrap();
    }
    root
}

fn args(v: &[&str], root: &Path) -> Query {
    let mut a: Vec<String> = v.iter().map(|s| (*s).to_owned()).collect();
    a.push(root.display().to_string());
    parse(&a).unwrap()
}

fn control(cancelled: &AtomicBool) -> Control<'_> {
    Control { cancelled, deadline: None, max_bytes: usize::MAX, max_records: 0 }
}

#[test]
fn prints_file_blocks_in_path_order_with_rg_exit_codes() {
    let root = tree("order");
    let flag = AtomicBool::new(false);
    let o = run(&args(&["-n", "-e", "hit 1", "--"], &root), &root, &control(&flag)).unwrap();
    let text = String::from_utf8(o.stdout).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let mut sorted = lines.clone();
    sorted.sort_unstable();
    assert_eq!(lines.len(), 11, "{text}");
    assert_eq!(lines, sorted);
    assert!(lines.iter().any(|l| l.ends_with("f1.txt:1:hit 1")), "{text}");
    assert_eq!(o.code, 0);
    let o = run(&args(&["-e", "nothing", "--"], &root), &root, &control(&flag)).unwrap();
    assert_eq!((o.code, o.stdout.len()), (1, 0));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cancellation_and_the_deadline_stop_the_walk() {
    let root = tree("stop");
    let cancelled = AtomicBool::new(true);
    let o = run(&args(&["-e", "hit", "--"], &root), &root, &control(&cancelled)).unwrap();
    assert!(o.cancelled && o.stdout.is_empty() && o.code == 1);
    let flag = AtomicBool::new(false);
    let past = Control { cancelled: &flag, deadline: Some(Instant::now() - Duration::from_millis(1)), max_bytes: usize::MAX, max_records: 0 };
    let o = run(&args(&["-e", "hit", "--"], &root), &root, &past).unwrap();
    assert!(o.timed_out && o.stdout.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn output_limits_truncate_and_stop() {
    let root = tree("limit");
    let flag = AtomicBool::new(false);
    let bytes = Control { cancelled: &flag, deadline: None, max_bytes: 30, max_records: 0 };
    let o = run(&args(&["-e", "hit", "--"], &root), &root, &bytes).unwrap();
    assert!(o.truncated && o.stdout.len() == 30);
    let records = Control { cancelled: &flag, deadline: None, max_bytes: usize::MAX, max_records: 3 };
    let o = run(&args(&["--files", "--null", "--"], &root), &root, &records).unwrap();
    assert!(o.truncated && o.stdout.iter().filter(|&&b| b == 0).count() >= 3);
    let _ = std::fs::remove_dir_all(&root);
}
