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
            .then(|| lookup(&super::super::unescape(&call.name)))
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
