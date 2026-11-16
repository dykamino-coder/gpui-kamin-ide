//! Runs one query as ripgrep 15.2.0 would (`crates/core/flags/hiargs.rs` and
//! `main.rs` of ripgrep, with the flags of `args.rs`): the same walk, matcher,
//! searcher and printer configuration from ripgrep's own crates. Each file is
//! searched in parallel, as in ripgrep; its block of output is kept whole and
//! the blocks are joined in path order, so the result is deterministic where
//! ripgrep's own order is not.
//!
//! The walk is the one place where step 2 (a file list kept current by a
//! watcher) plugs in: `haystacks` yields the files to search either way.

use crate::args::{Mode, Query};
use grep_printer::{PathPrinterBuilder, Standard, StandardBuilder, Summary, SummaryBuilder, SummaryKind};
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_matcher::LineTerminator;
use grep_searcher::{BinaryDetection, MmapChoice, Searcher, SearcherBuilder};
use ignore::{DirEntry, WalkBuilder, WalkState, overrides::OverrideBuilder, types::TypesBuilder};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;
use termcolor::NoColor;

/// Limits and cancellation of one request.
pub struct Control<'a> {
    pub cancelled: &'a AtomicBool,
    pub deadline: Option<Instant>,
    /// Output bytes kept; past it the search stops and reports `truncated`.
    pub max_bytes: usize,
    /// Output records (lines, or paths with `--null`) kept; 0 = no limit.
    pub max_records: usize,
}

/// What ripgrep would have printed and its exit code.
#[derive(Debug, Default)]
pub struct Outcome {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub code: i32,
    pub truncated: bool,
    pub timed_out: bool,
    pub cancelled: bool,
}

/// Shared state of the workers.
struct Shared<'a> {
    control: &'a Control<'a>,
    blocks: Mutex<Vec<(PathBuf, Vec<u8>)>>,
    stderr: Mutex<String>,
    bytes: AtomicUsize,
    records: AtomicUsize,
    matched: AtomicBool,
    errored: AtomicBool,
    truncated: AtomicBool,
    timed_out: AtomicBool,
    terminator: u8,
}

impl Shared<'_> {
    fn message(&self, text: String, error: bool) {
        if error {
            self.errored.store(true, Ordering::SeqCst);
        }
        if let Ok(mut e) = self.stderr.lock()
            && e.len() < 64 * 1024
        {
            e.push_str(&text);
            e.push('\n');
        }
    }

    /// Whether the walk must stop now.
    fn stop(&self) -> bool {
        if self.control.cancelled.load(Ordering::SeqCst) {
            return true;
        }
        if self.control.deadline.is_some_and(|d| Instant::now() >= d) {
            self.timed_out.store(true, Ordering::SeqCst);
            return true;
        }
        self.truncated.load(Ordering::SeqCst)
    }

    fn keep(&self, path: &Path, block: Vec<u8>) {
        if block.is_empty() {
            return;
        }
        let total = self.bytes.fetch_add(block.len(), Ordering::SeqCst) + block.len();
        let lines = block.iter().filter(|&&b| b == self.terminator).count();
        let records = self.records.fetch_add(lines, Ordering::SeqCst) + lines;
        if total > self.control.max_bytes || (self.control.max_records > 0 && records > self.control.max_records) {
            self.truncated.store(true, Ordering::SeqCst);
        }
        if let Ok(mut b) = self.blocks.lock() {
            b.push((path.to_path_buf(), block));
        }
    }
}

/// ripgrep's choice: one thread for a single file, else min(cores, 12).
fn threads(q: &Query) -> usize {
    if let Some(n) = q.threads {
        return n;
    }
    if q.paths.len() == 1 && !q.paths[0].is_dir() {
        return 1;
    }
    std::thread::available_parallelism().map_or(1, |n| n.get()).min(12)
}

fn walk_builder(q: &Query, cwd: &Path) -> Result<WalkBuilder, String> {
    let mut types = TypesBuilder::new();
    types.add_defaults();
    for t in &q.types {
        types.select(t);
    }
    let types = types.build().map_err(|e| e.to_string())?;
    let overrides = if q.globs.is_empty() {
        ignore::overrides::Override::empty()
    } else {
        let mut b = OverrideBuilder::new(cwd);
        for g in &q.globs {
            b.add(g).map_err(|e| e.to_string())?;
        }
        b.build().map_err(|e| e.to_string())?
    };
    let mut builder = WalkBuilder::new(&q.paths[0]);
    for p in q.paths.iter().skip(1) {
        builder.add(p);
    }
    let no_ignore = q.no_ignore;
    builder
        .follow_links(q.follow)
        .threads(threads(q))
        .skip_stdout(q.mode != Mode::Files)
        .overrides(overrides)
        .types(types)
        .hidden(!q.hidden)
        .parents(!no_ignore)
        .ignore(!no_ignore)
        .git_global(!no_ignore)
        .git_ignore(!no_ignore)
        .git_exclude(!no_ignore)
        .require_git(true)
        .current_dir(cwd);
    if !no_ignore {
        builder.add_custom_ignore_filename(".rgignore");
    }
    Ok(builder)
}

fn matcher(q: &Query) -> Result<RegexMatcher, String> {
    let mut b = RegexMatcherBuilder::new();
    b.multi_line(true).unicode(true).octal(false).fixed_strings(false).case_insensitive(q.case_insensitive);
    if q.multiline {
        b.dot_matches_new_line(q.multiline_dotall);
    } else {
        b.line_terminator(Some(b'\n')).dot_matches_new_line(false);
    }
    b.ban_byte(Some(b'\x00'));
    b.build_many(&q.patterns).map_err(|e| e.to_string())
}

fn searcher(q: &Query) -> Searcher {
    let line_number = q.line_number.unwrap_or(false) && q.mode == Mode::Standard;
    let mut b = SearcherBuilder::new();
    b.line_terminator(LineTerminator::byte(b'\n'))
        .line_number(line_number)
        .multi_line(q.multiline)
        .memory_map(MmapChoice::never());
    if q.mode == Mode::Standard {
        b.before_context(q.before_context).after_context(q.after_context);
    }
    b.build()
}

/// The entry is searched as ripgrep's haystack filter decides: an explicit
/// non-directory, or a file found by the walk.
fn haystack(dent: &DirEntry, shared: &Shared<'_>) -> Option<bool> {
    if let Some(err) = dent.error() {
        // ripgrep's ignore messages (a partial ignore file) do not count as errors.
        shared.message(err.to_string(), false);
    }
    let is_dir = dent.file_type().is_some_and(|ft| ft.is_dir()) || (dent.path_is_symlink() && dent.path().is_dir());
    if dent.depth() == 0 && !is_dir {
        return Some(true);
    }
    dent.file_type().is_some_and(|ft| ft.is_file()).then_some(false)
}

/// Runs `q` with the working directory `cwd`; Err means "let ripgrep run".
pub fn run(q: &Query, cwd: &Path, control: &Control<'_>) -> Result<Outcome, String> {
    let walker = walk_builder(q, cwd)?;
    let shared = Shared {
        control,
        blocks: Mutex::new(Vec::new()),
        stderr: Mutex::new(String::new()),
        bytes: AtomicUsize::new(0),
        records: AtomicUsize::new(0),
        matched: AtomicBool::new(false),
        errored: AtomicBool::new(false),
        truncated: AtomicBool::new(false),
        timed_out: AtomicBool::new(false),
        terminator: if q.mode == Mode::Files && q.null { b'\0' } else { b'\n' },
    };
    let with_filename = !(q.paths.len() == 1 && !q.paths[0].is_dir());
    let mut file_separator: Option<Vec<u8>> = None;
    if q.mode == Mode::Files {
        let mut printer = PathPrinterBuilder::new();
        printer.terminator(if q.null { b'\0' } else { b'\n' });
        let printer = &printer;
        let shared = &shared;
        walker.build_parallel().run(|| {
            // One printer per thread: building one sets up its hyperlink
            // environment, which costs more than printing a path.
            let buf = SharedBuf::default();
            let mut p = printer.build(NoColor::new(buf.clone()));
            Box::new(move |result| {
                if shared.stop() {
                    return WalkState::Quit;
                }
                let dent = match result {
                    Ok(d) => d,
                    Err(err) => {
                        shared.message(err.to_string(), true);
                        return WalkState::Continue;
                    }
                };
                if haystack(&dent, shared).is_none() {
                    return WalkState::Continue;
                }
                shared.matched.store(true, Ordering::SeqCst);
                if let Err(err) = p.write(dent.path()) {
                    shared.message(err.to_string(), true);
                }
                shared.keep(dent.path(), buf.take());
                WalkState::Continue
            })
        });
    } else {
        let matcher = matcher(q)?;
        let max_columns = q.max_columns.filter(|&n| n > 0);
        if q.mode == Mode::Standard && (q.before_context > 0 || q.after_context > 0) {
            file_separator = Some(b"--".to_vec());
        }
        let base = searcher(q);
        let (matcher, base, shared) = (&matcher, &base, &shared);
        walker.build_parallel().run(|| {
            let mut searcher = base.clone();
            let mut printer = Printer::new(q, with_filename, max_columns);
            Box::new(move |result| {
                if shared.stop() {
                    return WalkState::Quit;
                }
                let dent = match result {
                    Ok(d) => d,
                    Err(err) => {
                        shared.message(err.to_string(), true);
                        return WalkState::Continue;
                    }
                };
                let Some(explicit) = haystack(&dent, shared) else { return WalkState::Continue };
                searcher.set_binary_detection(if explicit { BinaryDetection::convert(b'\x00') } else { BinaryDetection::quit(b'\x00') });
                let path = dent.path();
                let found = printer.search(matcher, &mut searcher, path);
                match found {
                    Ok((matched, block)) => {
                        if matched {
                            shared.matched.store(true, Ordering::SeqCst);
                        }
                        shared.keep(path, block);
                    }
                    Err(err) => shared.message(format!("{}: {}", path.display(), err), true),
                }
                WalkState::Continue
            })
        });
    }
    let mut blocks = shared.blocks.into_inner().unwrap_or_default();
    // Path order by bytes: deterministic, and much cheaper than comparing
    // path components.
    blocks.sort_by(|a, b| a.0.as_os_str().cmp(b.0.as_os_str()));
    let mut stdout = Vec::new();
    for (_, block) in blocks {
        if let Some(sep) = &file_separator
            && !stdout.is_empty()
        {
            stdout.extend_from_slice(sep);
            stdout.push(b'\n');
        }
        stdout.extend_from_slice(&block);
    }
    let truncated = shared.truncated.load(Ordering::SeqCst);
    if truncated {
        stdout.truncate(control.max_bytes);
    }
    let matched = shared.matched.load(Ordering::SeqCst);
    let errored = shared.errored.load(Ordering::SeqCst);
    Ok(Outcome {
        stdout,
        stderr: shared.stderr.into_inner().unwrap_or_default(),
        code: if matched && !errored { 0 } else if errored { 2 } else { 1 },
        truncated,
        timed_out: shared.timed_out.load(Ordering::SeqCst),
        cancelled: control.cancelled.load(Ordering::SeqCst),
    })
}

/// A `Vec` that a printer writes into and the worker drains after each file.
#[derive(Clone, Default)]
struct SharedBuf(std::sync::Arc<Mutex<Vec<u8>>>);

impl SharedBuf {
    fn take(&self) -> Vec<u8> {
        self.0.lock().map(|mut b| std::mem::take(&mut *b)).unwrap_or_default()
    }
}

impl std::io::Write for SharedBuf {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut b) = self.0.lock() {
            b.extend_from_slice(data);
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The printer of one worker thread, configured as ripgrep's `printer()`.
enum Printer {
    Standard(Standard<NoColor<Vec<u8>>>),
    Summary(Summary<NoColor<Vec<u8>>>),
}

impl Printer {
    fn new(q: &Query, with_filename: bool, max_columns: Option<u64>) -> Printer {
        if q.mode == Mode::Standard {
            return Printer::Standard(
                StandardBuilder::new()
                    .max_columns(max_columns)
                    .path(with_filename)
                    .per_match_one_line(true)
                    .separator_context(Some(b"--".to_vec()))
                    .separator_field_context(b"-".to_vec())
                    .separator_field_match(b":".to_vec())
                    .build(NoColor::new(Vec::new())),
            );
        }
        let kind = if q.mode == Mode::Count { SummaryKind::Count } else { SummaryKind::PathWithMatch };
        Printer::Summary(
            SummaryBuilder::new()
                .exclude_zero(true)
                .kind(kind)
                .path(with_filename)
                .separator_field(b":".to_vec())
                .build(NoColor::new(Vec::new())),
        )
    }

    /// One file's block of output and whether it matched.
    fn search(&mut self, matcher: &RegexMatcher, searcher: &mut Searcher, path: &Path) -> std::io::Result<(bool, Vec<u8>)> {
        let matched = match self {
            Printer::Standard(p) => {
                p.get_mut().get_mut().clear();
                let mut sink = p.sink_with_path(matcher, path);
                searcher.search_path(matcher, path, &mut sink)?;
                sink.has_match()
            }
            Printer::Summary(p) => {
                p.get_mut().get_mut().clear();
                let mut sink = p.sink_with_path(matcher, path);
                searcher.search_path(matcher, path, &mut sink)?;
                sink.has_match()
            }
        };
        let out = match self {
            Printer::Standard(p) => std::mem::take(p.get_mut().get_mut()),
            Printer::Summary(p) => std::mem::take(p.get_mut().get_mut()),
        };
        Ok((matched, out))
    }
}
#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
