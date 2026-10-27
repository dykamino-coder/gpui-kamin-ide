//! Логический допуск CSS до AST: консервативные лексические единицы и раскрытие nesting.
//! Строки, url и escape имеют канонические границы; синтаксис здесь не отвергается.

use super::{at_url, escape_end, try_skip_string, try_skip_url};
use crate::budget::{
    BudgetKind,
    admission::{Admission, Buffer, Error},
};
use BudgetKind::{
    CssBytes, CssComponents, CssDeclarations, CssDepth, CssRules, CssSelectors, CssTokens,
};

mod expansion;
pub(crate) mod selector_depth;

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Counts(pub(crate) [usize; BudgetKind::COUNT]);

impl Counts {
    pub(crate) fn add(&mut self, other: Self) -> Result<(), Error> {
        for kind in BudgetKind::ALL {
            let i = kind as usize;
            self.0[i] = if kind == CssDepth {
                self.0[i].max(other.0[i])
            } else {
                self.0[i]
                    .checked_add(other.0[i])
                    .ok_or(Error::CapacityOverflow)?
            };
        }
        Ok(())
    }

    /// Копия допущенного AST оплачивает исходные единицы до клонирования.
    pub(crate) fn replay(&self, admission: &Admission) -> Result<(), Error> {
        admission.work(self.0[CssBytes as usize])?;
        for kind in [
            CssBytes,
            CssTokens,
            CssRules,
            CssSelectors,
            CssComponents,
            CssDeclarations,
            CssDepth,
        ] {
            if kind == CssDepth {
                admission.maximum(kind, self.0[kind as usize])?;
            } else {
                admission.consume(kind, self.0[kind as usize])?;
            }
        }
        Ok(())
    }
}

/// Независимый CSS entrypoint получает те же конечные defaults и свой ledger.
pub(crate) fn standalone(css: &str) -> Result<Admission, Error> {
    let budget = crate::budget::DocumentBudget::new(Default::default(), Default::default())
        .map_err(Error::Budget)?;
    let revision = budget.revision();
    revision.source("", css).map_err(Error::Budget)?;
    let admission = Admission::new(revision);
    admit(css, &admission)?;
    Ok(admission)
}

#[derive(Clone, Copy)]
struct Frame {
    close: char,
    expanded: usize,
    statement: usize,
    chain: usize,
    width: usize,
}

/// Консервативные лексические кандидаты покрывают и восстановление синтаксиса.
/// Потребители AST запускаются только после успешного допуска всей таблицы.
pub(crate) fn admit(css: &str, admission: &Admission) -> Result<Counts, Error> {
    let mut scan = Scan {
        css,
        admission,
        counts: Counts::default(),
        stack: Buffer::default(),
        at: 0,
        statement: 0,
        expanded: 0,
        chain: 0,
        width: 1,
    };
    scan.charge(CssBytes, css.len())?;
    while scan.at < css.len() {
        scan.step()?;
    }
    Ok(scan.counts)
}

struct Scan<'a> {
    css: &'a str,
    admission: &'a Admission,
    counts: Counts,
    stack: Buffer<Frame>,
    at: usize,
    statement: usize,
    expanded: usize,
    chain: usize,
    width: usize,
}

impl Scan<'_> {
    fn charge(&mut self, kind: BudgetKind, n: usize) -> Result<(), Error> {
        self.admission.consume(kind, n)?;
        self.counts.0[kind as usize] = self.counts.0[kind as usize]
            .checked_add(n)
            .ok_or(Error::CapacityOverflow)?;
        Ok(())
    }

    fn step(&mut self) -> Result<(), Error> {
        self.admission.work(1)?;
        let tail = &self.css[self.at..];
        let ch = tail.chars().next().unwrap();
        if tail.starts_with("/*") {
            self.at += 2;
            while self.at < self.css.len() {
                self.admission.work(1)?;
                if self.css[self.at..].starts_with("*/") {
                    self.at += 2;
                    break;
                }
                self.at += self.css[self.at..].chars().next().unwrap().len_utf8();
            }
            return Ok(());
        }
        if ch.is_ascii_whitespace() {
            self.charge(CssComponents, self.width)?;
            self.at += ch.len_utf8();
            return Ok(());
        }
        self.charge(CssTokens, 1)?;
        self.charge(CssComponents, self.width)?;
        if ch == '"' || ch == '\'' {
            self.at += ch.len_utf8();
            self.at += try_skip_string(&self.css[self.at..], ch, self.admission)?;
            self.charge_payload(tail.len() - (self.css.len() - self.at))?;
            return Ok(());
        }
        if at_url(tail) {
            self.at += try_skip_url(tail, self.admission)?;
            self.charge_payload(tail.len() - (self.css.len() - self.at))?;
            return Ok(());
        }
        if ch == '\\' {
            self.admission.work(9.min(tail.len()))?;
            self.at = escape_end(self.css, self.at);
            self.charge_payload(tail.len() - (self.css.len() - self.at))?;
            return Ok(());
        }
        match ch {
            '@' => self.charge(CssRules, 1)?,
            ',' => self.charge(CssSelectors, 1)?,
            ':' | ';' => {
                self.charge(CssDeclarations, 1)?;
                // Серия деклараций может копировать раскрытый список селекторов.
                self.charge(CssComponents, self.expanded)?;
            }
            _ => {}
        }
        if matches!(ch, '{' | ';') {
            let head = &self.css[self.statement..self.at];
            self.admission.work(head.len())?;
            self.charge(CssComponents, expansion::registry(head, self.expanded)?)?;
        }
        match ch {
            '{' | '(' | '[' => {
                let depth = self
                    .stack
                    .len()
                    .checked_add(1)
                    .ok_or(Error::CapacityOverflow)?;
                self.admission.maximum(CssDepth, depth)?;
                self.counts.0[CssDepth as usize] = self.counts.0[CssDepth as usize].max(depth);
                let frame = Frame {
                    close: match ch {
                        '{' => '}',
                        '(' => ')',
                        _ => ']',
                    },
                    expanded: self.expanded,
                    statement: self.statement,
                    chain: self.chain,
                    width: self.width,
                };
                if ch == '{' {
                    self.charge(CssRules, 1)?;
                    self.charge(CssSelectors, 1)?;
                    let head = &self.css[self.statement..self.at];
                    self.admission.work(head.len())?;
                    let (expanded, width, amps) = expansion::selectors(head, self.expanded)?;
                    let own_chain = selector_depth::measure(head, self.admission)?;
                    let chain = self
                        .chain
                        .checked_mul(amps.max(1))
                        .and_then(|n| n.checked_add(own_chain))
                        .ok_or(Error::CapacityOverflow)?;
                    self.admission.maximum(CssDepth, chain)?;
                    self.counts.0[CssDepth as usize] = self.counts.0[CssDepth as usize].max(chain);
                    self.chain = chain;
                    self.charge(CssComponents, expanded)?;
                    self.expanded = expanded;
                    self.width = self.width.max(width);
                    self.statement = self.at + 1;
                }
                self.stack.push(frame, self.admission)?;
            }
            ')' | ']' | '}' if self.stack.last().is_some_and(|f| f.close == ch) => {
                let frame = self.stack.pop().unwrap();
                self.expanded = frame.expanded;
                self.chain = frame.chain;
                self.width = frame.width;
                self.statement = if ch == '}' {
                    self.at + 1
                } else {
                    frame.statement
                };
            }
            ';' => self.statement = self.at + 1,
            _ => {}
        }
        self.at += ch.len_utf8();
        Ok(())
    }

    fn charge_payload(&mut self, bytes: usize) -> Result<(), Error> {
        let copies = bytes
            .checked_mul(self.width)
            .ok_or(Error::CapacityOverflow)?;
        self.charge(CssComponents, copies)
    }
}

#[cfg(test)]
mod tests;
