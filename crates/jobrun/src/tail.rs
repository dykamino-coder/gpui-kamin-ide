//! Splits the runner's own command line into the program to start and the
//! untouched rest of the line.

const QUOTE: u16 = b'"' as u16;
const BACKSLASH: u16 = b'\\' as u16;

fn is_space(unit: u16) -> bool {
    unit == u16::from(b' ') || unit == u16::from(b'\t')
}

fn skip_spaces(line: &[u16]) -> &[u16] {
    let start = line
        .iter()
        .position(|&unit| !is_space(unit))
        .unwrap_or(line.len());
    &line[start..]
}

/// The first token of a command line names the executable and has its own
/// rule: a quoted name ends at the next quote, a bare one at the next space.
fn skip_executable(line: &[u16]) -> &[u16] {
    let end = if line.first() == Some(&QUOTE) {
        line[1..]
            .iter()
            .position(|&unit| unit == QUOTE)
            .map_or(line.len(), |at| at + 2)
    } else {
        line.iter()
            .position(|&unit| is_space(unit))
            .unwrap_or(line.len())
    };
    &line[end..]
}

/// Read one argument by the standard Windows rule (backslashes are literal
/// unless they precede a quote) and return it with the rest of the line.
fn argument(line: &[u16]) -> (Vec<u16>, &[u16]) {
    let mut value = Vec::new();
    let mut quoted = false;
    let mut at = 0;
    while at < line.len() {
        let unit = line[at];
        if unit == BACKSLASH {
            let run = line[at..]
                .iter()
                .take_while(|&&next| next == BACKSLASH)
                .count();
            if line.get(at + run) == Some(&QUOTE) {
                value.extend(std::iter::repeat_n(BACKSLASH, run / 2));
                if run % 2 == 1 {
                    value.push(QUOTE);
                    at += run + 1;
                } else {
                    at += run;
                }
            } else {
                value.extend(std::iter::repeat_n(BACKSLASH, run));
                at += run;
            }
        } else if unit == QUOTE {
            quoted = !quoted;
            at += 1;
        } else if is_space(unit) && !quoted {
            break;
        } else {
            value.push(unit);
            at += 1;
        }
    }
    (value, &line[at..])
}

/// `runner.exe program rest of the line` gives `program` and `rest of the
/// line`. `None` when no program is named.
pub fn command(line: &[u16]) -> Option<(Vec<u16>, &[u16])> {
    let after_runner = skip_spaces(skip_executable(line));
    if after_runner.is_empty() {
        return None;
    }
    let (program, rest) = argument(after_runner);
    Some((program, skip_spaces(rest)))
}

#[cfg(test)]
mod tests {
    use super::command;

    fn split(line: &str) -> Option<(String, String)> {
        let wide: Vec<u16> = line.encode_utf16().collect();
        command(&wide).map(|(program, rest)| {
            (
                String::from_utf16_lossy(&program),
                String::from_utf16_lossy(rest),
            )
        })
    }

    fn pair(program: &str, rest: &str) -> Option<(String, String)> {
        Some((program.to_owned(), rest.to_owned()))
    }

    #[test]
    fn keeps_the_rest_of_the_line_untouched() {
        assert_eq!(
            split(r#"kamin-jobrun.exe bash.exe -c "echo \"a  b\" 'c'""#),
            pair("bash.exe", r#"-c "echo \"a  b\" 'c'""#)
        );
    }

    #[test]
    fn reads_quoted_runner_and_program_paths() {
        assert_eq!(
            split(
                r#""C:\Program Files\K\kamin-jobrun.exe"  "C:\Program Files\Git\bin\bash.exe" -c x"#
            ),
            pair(r"C:\Program Files\Git\bin\bash.exe", "-c x")
        );
    }

    #[test]
    fn follows_the_backslash_rule_before_a_quote() {
        assert_eq!(split(r#"r "C:\a b\\" tail"#), pair(r"C:\a b\", "tail"));
        assert_eq!(split(r#"r a\"b c"#), pair(r#"a"b"#, "c"));
        assert_eq!(split(r"r C:\plain\path"), pair(r"C:\plain\path", ""));
    }

    #[test]
    fn reports_a_missing_program() {
        assert_eq!(split("kamin-jobrun.exe"), None);
        assert_eq!(split(r#""C:\x y\kamin-jobrun.exe"   "#), None);
        assert_eq!(split("r --probe"), pair("--probe", ""));
    }
}
