//! Ограниченная подстановка CSS var(): размер результата одного значения, работа одного
//! значения или DAG пользовательских свойств элемента и aggregate ревизии ограничены
//! независимо от глубины DAG. CSS Variables §3.3 разрешает считать чрезмерно длинное
//! значение негодным в момент вычисления; причина отказа остаётся типизированной.

use super::{Unsubstituted, Work};
mod expansion;
pub(super) use expansion::{Expansion, Step};

pub(super) struct Call {
    start: usize,
    end: usize,
    name: String,
    fallback: Option<String>,
}

pub fn has_var(raw: &str) -> bool {
    !calls(raw).is_empty()
}

/// Only selected fallback arguments are substituted (CSS Variables §3).
/// Whitespace preserves token boundaries: substituting `20` before `px`
/// must not manufacture a dimension token that was absent from the input.
/// Значение без `var()` не подставляется и работы не тратит; предел длины тот же.
#[cfg(test)]
pub fn substitute(
    raw: &str,
    lookup: &mut impl FnMut(&str) -> Option<String>,
    work: &Work<'_>,
) -> Result<String, Unsubstituted> {
    let calls = calls(raw);
    if calls.is_empty() {
        work.fit(0, raw.len())?;
        return Ok(raw.to_string());
    }
    let mut expansion = Expansion::new(raw.to_string(), calls, work)?;
    loop {
        match expansion.step(work)? {
            Step::Need(name) => expansion.supply(lookup(&name), work)?,
            Step::Done(value) => return Ok(value),
        }
    }
}

/// Проверить сериализованный идентификатор до декодирования при поиске.
/// Экранированный пробел допустим в имени, а отдельный токен пробела — нет.
fn custom_name(name: &str) -> bool {
    let decoded = super::super::unescape(name);
    decoded.starts_with("--") && decoded != "--" && super::super::selector_tokens::ident(name)
}

fn calls(raw: &str) -> Vec<Call> {
    use super::super::variable_tokens::block_end;
    let mut found = Vec::new();
    let mut from = 0;
    for function in crate::css_syntax::functions(raw) {
        if function.start < from || !function.name.eq_ignore_ascii_case("var") {
            continue;
        }
        let Some(end) = block_end(raw, function.open, '(') else {
            break;
        };
        let inner = super::super::strip_comments(&raw[function.open + 1..end]);
        let args = super::super::split_top_level(&inner, ',');
        from = (end + 1).min(raw.len());
        found.push(Call {
            start: function.start,
            end: from,
            name: args[0].trim().to_string(),
            fallback: (args.len() > 1).then(|| args[1..].join(",")),
        });
    }
    found
}
