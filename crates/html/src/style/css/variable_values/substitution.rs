//! Ограниченная подстановка CSS var(): размер результата и суммарная работа
//! выбранных запасных значений ограничены независимо от глубины DAG.
//! CSS Variables §3.3 разрешает считать чрезмерно длинное значение негодным.

// Пределы в байтах UTF-8; бюджет принадлежит одному вычисляемому значению.
const MAX_OUTPUT: usize = 1 << 20;
const MAX_WORK: usize = 32 << 20;

struct Budget {
    work: usize,
}

impl Budget {
    fn spend(&mut self, bytes: usize) -> Option<()> {
        self.work = self.work.checked_sub(bytes)?;
        Some(())
    }

    fn append(&mut self, result: &mut String, value: &str) -> Option<()> {
        if value.len() > MAX_OUTPUT.checked_sub(result.len())? {
            return None;
        }
        self.spend(value.len())?;
        result.push_str(value);
        Some(())
    }
}

struct Call {
    start: usize,
    end: usize,
    name: String,
    fallback: Option<String>,
}

pub(crate) fn has_var(raw: &str) -> bool {
    !calls(raw).is_empty()
}

/// Only selected fallback arguments are substituted (CSS Variables §3).
/// Whitespace preserves token boundaries: substituting `20` before `px`
/// must not manufacture a dimension token that was absent from the input.
pub(crate) fn substitute(
    raw: &str,
    lookup: &mut impl FnMut(&str) -> Option<String>,
) -> Option<String> {
    expand(raw, lookup, &mut Budget { work: MAX_WORK })
}

fn expand(
    raw: &str,
    lookup: &mut impl FnMut(&str) -> Option<String>,
    budget: &mut Budget,
) -> Option<String> {
    // Сканер проходит аргументы несколько раз и создаёт строки запасных значений.
    budget.spend(raw.len().checked_mul(8)?)?;
    let mut result = String::new();
    let mut from = 0;
    for call in calls(raw) {
        budget.append(&mut result, &raw[from..call.start])?;
        // css-variables-2 §3 (resolve a var() function): a first argument
        // that is not a <custom-property-name> yields the guaranteed-invalid
        // value, so the fallback (if any) is used.
        let value = custom_name(&call.name)
            .then(|| lookup(&call.name))
            .flatten();
        let replacement = match value {
            Some(value) => value,
            None => expand(call.fallback.as_deref()?, lookup, budget)?,
        };
        budget.append(&mut result, " ")?;
        budget.append(&mut result, &replacement)?;
        budget.append(&mut result, " ")?;
        from = call.end;
    }
    budget.append(&mut result, &raw[from..])?;
    Some(result)
}

/// Names arrive with escapes already decoded, so only the dashed prefix is
/// checked here; any other token sequence simply misses the lookup.
fn custom_name(name: &str) -> bool {
    name.starts_with("--") && name != "--"
}

fn calls(raw: &str) -> Vec<Call> {
    use super::super::variable_tokens::{block_end, escape_end, string_end};
    let mut found = Vec::new();
    let mut at = 0;
    while at < raw.len() {
        let ch = raw[at..].chars().next().unwrap();
        if matches!(ch, '\'' | '"') {
            at = string_end(raw, at, ch).unwrap_or(raw.len());
            continue;
        }
        if raw[at..].starts_with("/*") {
            at += raw[at + 2..].find("*/").map_or(raw.len() - at, |n| n + 4);
            continue;
        }
        let start = at;
        if ch == '\\' || ch == '-' || ch == '_' || ch.is_alphanumeric() || ch >= '\u{80}' {
            while at < raw.len() {
                let c = raw[at..].chars().next().unwrap();
                if c == '\\' {
                    at = escape_end(raw, at);
                } else if c.is_alphanumeric() || matches!(c, '-' | '_') || c >= '\u{80}' {
                    at += c.len_utf8();
                } else {
                    break;
                }
            }
            if raw[start..at].eq_ignore_ascii_case("var") && raw[at..].starts_with('(') {
                let Some(end) = block_end(raw, at, '(') else {
                    break;
                };
                let inner = super::super::strip_comments(&raw[at + 1..end]);
                let args = super::super::split_top_level(&inner, ',');
                found.push(Call {
                    start,
                    end: (end + 1).min(raw.len()),
                    // parse_decls has already decoded identifier escapes.
                    name: args[0].trim().to_string(),
                    fallback: (args.len() > 1).then(|| args[1..].join(",")),
                });
                at = (end + 1).min(raw.len());
            }
        }
        if at == start {
            at += ch.len_utf8();
        }
    }
    found
}
