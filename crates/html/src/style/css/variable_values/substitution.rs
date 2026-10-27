//! Ограниченная подстановка CSS var(): размер результата одного значения, работа одного
//! значения или DAG пользовательских свойств элемента и aggregate ревизии ограничены
//! независимо от глубины DAG. CSS Variables §3.3 разрешает считать чрезмерно длинное
//! значение негодным в момент вычисления; причина отказа остаётся типизированной.

use crate::budget::{BudgetError, BudgetKind, RevisionBudget};
use std::cell::Cell;

// Пределы в байтах UTF-8 из неизменяемого профиля.
const MAX_OUTPUT: usize = crate::budget::DocumentLimits::VAR_OUTPUT_BYTES as usize;
const MAX_WORK: usize = crate::budget::DocumentLimits::VAR_WORK_BYTES as usize;

/// Какой предел сделал подстановку негодной.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubstitutionLimit {
    /// Результат одного значения длиннее `limit` байт (CSS Variables §3.3).
    Output { limit: u64, attempted: u64 },
    /// Работа одного значения или DAG пользовательских свойств элемента.
    Work { limit: u64 },
    /// Aggregate подстановок ревизии исчерпан; ревизия завершается отказом.
    Revision(BudgetError),
}

/// Почему значения нет: обычное guaranteed-invalid или предел реализации.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unsubstituted {
    Invalid,
    Limit(SubstitutionLimit),
}

/// Бюджет работы одной области подстановки: значения обычного свойства или всего DAG
/// пользовательских свойств элемента. Каждая единица списывается и с ревизии.
pub(crate) struct Work<'a> {
    left: Cell<usize>,
    revision: &'a RevisionBudget,
    /// Первый предел области: одна причина для журнала документа.
    first: Cell<Option<SubstitutionLimit>>,
}

impl<'a> Work<'a> {
    pub(crate) fn new(revision: &'a RevisionBudget) -> Self {
        Self {
            left: Cell::new(MAX_WORK),
            revision,
            first: Cell::new(None),
        }
    }

    /// Записать первый предел области в журнал документа (invalid at computed time
    /// со штатной семантикой уже применён вызывающим).
    pub(crate) fn finish(self, doc: &crate::document::state::DocState) {
        if let Some(limit) = self.first.get() {
            doc.record_observed(diagnostic(limit));
        }
    }

    fn limit(&self, limit: SubstitutionLimit) -> Unsubstituted {
        if self.first.get().is_none() {
            self.first.set(Some(limit));
        }
        Unsubstituted::Limit(limit)
    }

    fn spend(&self, bytes: usize) -> Result<(), Unsubstituted> {
        let Some(left) = self.left.get().checked_sub(bytes) else {
            return Err(self.limit(SubstitutionLimit::Work {
                limit: MAX_WORK as u64,
            }));
        };
        self.revision
            .consume(BudgetKind::SubstitutionBytes, bytes as u64)
            .map_err(|error| self.limit(SubstitutionLimit::Revision(error)))?;
        self.left.set(left);
        Ok(())
    }

    fn append(&self, result: &mut String, value: &str) -> Result<(), Unsubstituted> {
        self.fit(result.len(), value.len())?;
        self.spend(value.len())?;
        result.push_str(value);
        Ok(())
    }

    fn fit(&self, len: usize, extra: usize) -> Result<(), Unsubstituted> {
        if extra > MAX_OUTPUT.saturating_sub(len) {
            return Err(self.limit(SubstitutionLimit::Output {
                limit: MAX_OUTPUT as u64,
                attempted: len as u64 + extra as u64,
            }));
        }
        Ok(())
    }
}

fn diagnostic(limit: SubstitutionLimit) -> crate::diagnostics::Diagnostic {
    use crate::diagnostics::{Code, Diagnostic, Measure, Phase, Severity};
    let measure = match limit {
        SubstitutionLimit::Output { limit, attempted } => Measure::Length { limit, attempted },
        SubstitutionLimit::Work { limit } => Measure::Ceiling { limit },
        SubstitutionLimit::Revision(error) => return Diagnostic::budget(Phase::Style, error),
    };
    Diagnostic::observed(
        Code::SubstitutionLimit,
        Severity::Warning,
        Phase::Style,
        Some(measure),
    )
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
/// Значение без `var()` не подставляется и работы не тратит; предел длины тот же.
pub(crate) fn substitute(
    raw: &str,
    lookup: &mut impl FnMut(&str) -> Option<String>,
    work: &Work<'_>,
) -> Result<String, Unsubstituted> {
    let calls = calls(raw);
    if calls.is_empty() {
        work.fit(0, raw.len())?;
        return Ok(raw.to_string());
    }
    expand(raw, calls, lookup, work)
}

fn expand(
    raw: &str,
    calls: Vec<Call>,
    lookup: &mut impl FnMut(&str) -> Option<String>,
    work: &Work<'_>,
) -> Result<String, Unsubstituted> {
    // Сканер проходит аргументы несколько раз и создаёт строки запасных значений.
    work.spend(raw.len().saturating_mul(8))?;
    let mut result = String::new();
    let mut from = 0;
    for call in calls {
        work.append(&mut result, &raw[from..call.start])?;
        // css-variables-2 §3 (resolve a var() function): a first argument
        // that is not a <custom-property-name> yields the guaranteed-invalid
        // value, so the fallback (if any) is used.
        let value = custom_name(&call.name)
            .then(|| lookup(&super::super::unescape(&call.name)))
            .flatten();
        let replacement = match value {
            Some(value) => value,
            None => {
                let fallback = call.fallback.as_deref().ok_or(Unsubstituted::Invalid)?;
                expand(fallback, self::calls(fallback), lookup, work)?
            }
        };
        work.append(&mut result, " ")?;
        work.append(&mut result, &replacement)?;
        work.append(&mut result, " ")?;
        from = call.end;
    }
    work.append(&mut result, &raw[from..])?;
    Ok(result)
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
