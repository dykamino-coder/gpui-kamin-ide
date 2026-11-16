//! Differential test against the ripgrep binary of the same version
//! (`KAMIN_RG_PATH`, ripgrep 15.2.0; CI provides it): random trees and command
//! lines of every flag `args.rs` accepts, each answered by `run` and by
//! ripgrep, compared file block by file block (ripgrep's own order of files
//! is arbitrary) and by exit code. The seed is printed; `KAMIN_SEARCH_SEED`
//! repeats a run. Without ripgrep the test is skipped locally and fails in CI.

use crate::args::parse;
use crate::run::{Control, run};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

#[allow(
    clippy::disallowed_methods,
    reason = "the test reads its configuration from the environment"
)]
fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// mulberry32, as the extension's differential test.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }

    fn below(&mut self, n: usize) -> usize {
        // `next` is in [0, 1); the product truncates to an index below n.
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let i = (self.next() * n as f64) as usize;
        i.min(n - 1)
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

/// Records of stdout (lines, or NUL-terminated paths), sorted: ripgrep's own
/// order of files is arbitrary.
fn blocks(out: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(out);
    let mut lines: Vec<&str> = text.split(['\n', '\0']).collect();
    lines.sort_unstable();
    lines.into_iter().map(str::to_owned).collect()
}

fn put(root: &Path, rel: &str, data: &[u8]) {
    let p = root.join(rel);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, data);
}

fn check(rg: &str, cwd: &Path, args: Vec<String>, label: &str) -> bool {
    let q = match parse(&args) {
        Ok(q) => q,
        Err(e) => panic!("{label}: the generator made an unsupported command: {e}"),
    };
    let cancelled = AtomicBool::new(false);
    let control = Control {
        cancelled: &cancelled,
        deadline: None,
        max_bytes: usize::MAX,
        max_records: 0,
    };
    let Ok(ours) = run(&q, cwd, &control) else {
        return false;
    };
    let theirs = Command::new(rg)
        .args(&args)
        .current_dir(cwd)
        .output()
        .expect("ripgrep runs");
    assert_eq!(
        (blocks(&ours.stdout), Some(ours.code)),
        (blocks(&theirs.stdout), theirs.status.code()),
        "{label}: {args:?}"
    );
    true
}

const NAMES: &[&str] = &[
    "a", "b.rs", "c.ts", "d.md", ".hidden", "e.txt", "build", "src", "x y.rs", "ü.ts", "[ab]",
];
const WORDS: &[&str] = &[
    "fn",
    "let",
    "export",
    "alpha",
    "Beta",
    "42",
    "слово",
    "(",
    ")",
    "{",
    "}",
    "  ",
    "café",
    "end",
];
const PATTERNS: &[&str] = &[
    "fn",
    "alpha|Beta",
    "\\w+\\(",
    "^let",
    "\\d+$",
    "(?i)beta",
    "слово",
    "a.b",
    "\\bend\\b",
    "[a-c]+",
    "x\\s+y",
    "\\n",
    "fn.*\\n.*let",
];
const GLOBS: &[&str] = &[
    "*.rs",
    "!*.md",
    "src/**",
    "!**/build/**",
    "*.{ts,rs}",
    "[ab]*",
    "!.hidden",
];
const TYPES: &[&str] = &["rust", "ts", "md", "txt"];
const IGNORES: &[&str] = &[
    "*.md",
    "build/",
    "!keep.md",
    "/a",
    "src/**/*.ts",
    "\\#x",
    "*.rs  ",
    "e.txt\r",
];

fn tree(r: &mut Rng, root: &Path) {
    let mut dirs = vec![String::new()];
    for _ in 0..1 + r.below(4) {
        let base = r.pick(&dirs).clone();
        let name = *r.pick(NAMES);
        dirs.push(if base.is_empty() {
            name.to_owned()
        } else {
            format!("{base}/{name}")
        });
    }
    for _ in 0..4 + r.below(12) {
        let mut text = String::new();
        for _ in 0..r.below(6) {
            for _ in 0..1 + r.below(5) {
                text.push_str(r.pick(WORDS));
                text.push(' ');
            }
            text.push_str(if r.below(5) == 0 { "\r\n" } else { "\n" });
        }
        let mut data = text.into_bytes();
        match r.below(12) {
            0 => data.extend_from_slice(b"\0bin\n"),
            1 => data.splice(0..0, [0xef, 0xbb, 0xbf]).for_each(drop),
            2 => data.extend(std::iter::repeat_n(b'x', 600)),
            _ => {}
        }
        let dir = r.pick(&dirs).clone();
        let name = *r.pick(NAMES);
        let rel = if dir.is_empty() {
            name.to_owned()
        } else {
            format!("{dir}/{name}")
        };
        put(root, &rel, &data);
    }
    if r.below(3) > 0 {
        let _ = std::fs::create_dir_all(root.join(".git"));
    }
    for d in &dirs {
        if r.below(3) == 0 {
            let file = *r.pick(&[".gitignore", ".ignore", ".rgignore"]);
            let lines: Vec<&str> = (0..1 + r.below(3)).map(|_| *r.pick(IGNORES)).collect();
            put(
                root,
                &if d.is_empty() {
                    file.to_owned()
                } else {
                    format!("{d}/{file}")
                },
                lines.join("\n").as_bytes(),
            );
        }
    }
}

fn grep_args(r: &mut Rng, path: &Path) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "--hidden".into(),
        "--max-columns".into(),
        "500".into(),
        "--glob".into(),
        "!.git".into(),
    ];
    match r.below(3) {
        0 => a.push("--files-with-matches".into()),
        1 => a.push("--count".into()),
        _ => {
            if r.below(2) == 0 {
                a.push("-n".into());
            }
            match r.below(4) {
                0 => a.extend(["-C".into(), r.below(3).to_string()]),
                1 => a.extend([
                    "-A".into(),
                    r.below(3).to_string(),
                    "-B".into(),
                    r.below(3).to_string(),
                ]),
                _ => {}
            }
        }
    }
    if r.below(4) == 0 {
        a.push("-i".into());
    }
    let pattern = (*r.pick(PATTERNS)).to_owned();
    if pattern.contains("\\n") || r.below(6) == 0 {
        a.extend(["-U".into(), "--multiline-dotall".into()]);
    }
    if r.below(3) == 0 {
        a.extend(["--glob".into(), (*r.pick(GLOBS)).to_owned()]);
    }
    if r.below(5) == 0 {
        a.extend(["--type".into(), (*r.pick(TYPES)).to_owned()]);
    }
    a.extend([
        "-e".into(),
        pattern,
        "--".into(),
        path.display().to_string(),
    ]);
    a
}

#[test]
fn equals_ripgrep_on_random_trees() {
    let Some(rg) = env("KAMIN_RG_PATH") else {
        assert!(
            env("CI").is_none(),
            "KAMIN_RG_PATH (ripgrep 15.2.0) is required in CI"
        );
        return;
    };
    let version = Command::new(&rg)
        .arg("--version")
        .output()
        .expect("ripgrep runs");
    assert!(
        String::from_utf8_lossy(&version.stdout).starts_with("ripgrep 15.2.0"),
        "the reference must be ripgrep 15.2.0"
    );
    let seed: u32 = env("KAMIN_SEARCH_SEED")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.subsec_nanos());
            now | 1
        });
    #[allow(clippy::print_stderr, reason = "the seed repeats a failing run")]
    {
        eprintln!("kamin-search differential seed: {seed}");
    }
    let base: PathBuf = std::env::temp_dir().join(format!("kamin-search-diff-{seed}"));
    let _ = std::fs::remove_dir_all(&base);
    let mut r = Rng(seed);
    let mut answered = 0;
    for t in 0..30 {
        let root = base.join(format!("t{t}"));
        tree(&mut r, &root);
        for k in 0..10 {
            let target = if k == 0 {
                root.join(r.pick(NAMES))
            } else {
                root.clone()
            };
            if !target.exists() {
                continue;
            }
            if check(
                &rg,
                &root,
                grep_args(&mut r, &target),
                &format!("seed {seed} tree {t}"),
            ) {
                answered += 1;
            }
            let glob = (*r.pick(GLOBS)).to_owned();
            let files: Vec<String> = [
                "--files",
                "--null",
                "--hidden",
                "--no-ignore",
                "--no-config",
                "--follow",
                "--glob",
                &glob,
                "--",
            ]
            .iter()
            .map(|s| (*s).to_owned())
            .chain([root.display().to_string()])
            .collect();
            if check(&rg, &root, files, &format!("seed {seed} tree {t} files")) {
                answered += 1;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    assert!(answered > 300, "too few commands answered: {answered}");
}
