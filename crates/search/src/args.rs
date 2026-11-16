//! The ripgrep command lines of the Bridge Grep and Glob tools, read into a
//! query. Only the flags those tools emit are accepted, with ripgrep's
//! meaning; any other argument makes the whole request unsupported, and the
//! extension runs `rg` itself.

use std::path::PathBuf;

/// What to print for each file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Matching lines (`rg PATTERN`).
    Standard,
    /// `--files-with-matches`.
    FilesWithMatches,
    /// `--count`.
    Count,
    /// `--files`: list the files that would be searched.
    Files,
}

/// One ripgrep invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub mode: Mode,
    pub patterns: Vec<String>,
    pub paths: Vec<PathBuf>,
    pub globs: Vec<String>,
    pub types: Vec<String>,
    pub hidden: bool,
    pub no_ignore: bool,
    pub follow: bool,
    pub null: bool,
    pub case_insensitive: bool,
    pub line_number: Option<bool>,
    pub multiline: bool,
    pub multiline_dotall: bool,
    pub before_context: usize,
    pub after_context: usize,
    pub max_columns: Option<u64>,
    pub threads: Option<usize>,
    /// `--no-config`: ripgrep would not read `RIPGREP_CONFIG_PATH`.
    pub no_config: bool,
}

fn number<T: std::str::FromStr>(flag: &str, value: Option<&String>) -> Result<T, String> {
    value
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| format!("{flag} needs a number"))
}

/// Parses `args`; Err names what is not replicated.
pub fn parse(args: &[String]) -> Result<Query, String> {
    let mut q = Query {
        mode: Mode::Standard,
        patterns: Vec::new(),
        paths: Vec::new(),
        globs: Vec::new(),
        types: Vec::new(),
        hidden: false,
        no_ignore: false,
        follow: false,
        null: false,
        case_insensitive: false,
        line_number: None,
        multiline: false,
        multiline_dotall: false,
        before_context: 0,
        after_context: 0,
        max_columns: None,
        threads: None,
        no_config: false,
    };
    let mut positional: Vec<String> = Vec::new();
    let mut it = args.iter();
    let mut rest = false;
    // As in ripgrep, -A and -B take precedence over -C in any order.
    let (mut before, mut after, mut both) = (None, None, None);
    while let Some(a) = it.next() {
        if rest {
            positional.push(a.clone());
            continue;
        }
        match a.as_str() {
            "--" => rest = true,
            "--hidden" => q.hidden = true,
            "--no-ignore" => q.no_ignore = true,
            "--follow" => q.follow = true,
            "--null" => q.null = true,
            "--no-config" => q.no_config = true,
            "--files" => q.mode = Mode::Files,
            "--files-with-matches" | "-l" => q.mode = Mode::FilesWithMatches,
            "--count" | "-c" => q.mode = Mode::Count,
            "-i" | "--ignore-case" => q.case_insensitive = true,
            "-n" | "--line-number" => q.line_number = Some(true),
            "-U" | "--multiline" => q.multiline = true,
            "--multiline-dotall" => q.multiline_dotall = true,
            "--max-columns" | "-M" => q.max_columns = Some(number(a, it.next())?),
            "-j" | "--threads" => q.threads = Some(number(a, it.next())?),
            "-A" | "--after-context" => after = Some(number(a, it.next())?),
            "-B" | "--before-context" => before = Some(number(a, it.next())?),
            "-C" | "--context" => both = Some(number(a, it.next())?),
            "--glob" | "-g" => q.globs.push(it.next().ok_or("--glob needs a value")?.clone()),
            "--type" | "-t" => q.types.push(it.next().ok_or("--type needs a value")?.clone()),
            "-e" | "--regexp" => q.patterns.push(it.next().ok_or("-e needs a value")?.clone()),
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("unsupported argument {other}"));
            }
            other => positional.push(other.to_owned()),
        }
    }
    if q.mode != Mode::Files && q.patterns.is_empty() {
        if positional.is_empty() {
            return Err("no pattern".into());
        }
        q.patterns.push(positional.remove(0));
    }
    if positional.is_empty() {
        // ripgrep would guess between stdin and ./ here.
        return Err("no path".into());
    }
    q.paths = positional.into_iter().map(PathBuf::from).collect();
    q.before_context = before.or(both).unwrap_or(0);
    q.after_context = after.or(both).unwrap_or(0);
    Ok(q)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn reads_the_grep_tool_command() {
        let q = parse(&s(&[
            "--hidden", "--max-columns", "500", "--glob", "!.git", "--count", "-i", "-U", "--multiline-dotall",
            "--glob", "*.rs", "--type", "rust", "-e", "-x", "--", "C:/w",
        ]))
        .unwrap();
        assert_eq!(q.mode, Mode::Count);
        assert_eq!(q.patterns, vec!["-x".to_owned()]);
        assert_eq!(q.globs, vec!["!.git".to_owned(), "*.rs".to_owned()]);
        assert_eq!(q.types, vec!["rust".to_owned()]);
        assert_eq!(q.max_columns, Some(500));
        assert!(q.hidden && q.case_insensitive && q.multiline && q.multiline_dotall);
        assert_eq!(q.paths, vec![PathBuf::from("C:/w")]);
    }

    #[test]
    fn reads_the_glob_tool_command_and_context() {
        let q = parse(&s(&["--files", "--null", "--hidden", "--no-ignore", "--no-config", "--follow", "--glob", "*.ts", "--", "/b"])).unwrap();
        assert_eq!(q.mode, Mode::Files);
        assert!(q.null && q.no_ignore && q.no_config && q.follow);
        let q = parse(&s(&["-C", "2", "-A", "1", "-n", "-e", "x", "--", "/b"])).unwrap();
        assert_eq!((q.before_context, q.after_context, q.line_number), (2, 1, Some(true)));
    }

    #[test]
    fn refuses_what_it_does_not_replicate() {
        for a in [&["-P", "-e", "x", "--", "/b"][..], &["-e", "x"], &["--sort", "path", "-e", "x", "--", "/b"], &["-A"]] {
            assert!(parse(&s(a)).is_err(), "{a:?}");
        }
    }
}
