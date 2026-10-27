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

mod selector_depth;

#[derive(Default, Debug)]
pub(crate) struct Counts(pub(crate) [usize; BudgetKind::COUNT]);

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
            self.at += ch.len_utf8();
            return Ok(());
        }
        self.charge(CssTokens, 1)?;
        self.charge(CssComponents, 1)?;
        if ch == '"' || ch == '\'' {
            self.at += ch.len_utf8();
            self.at += try_skip_string(&self.css[self.at..], ch, self.admission)?;
            return Ok(());
        }
        if at_url(tail) {
            self.at += try_skip_url(tail, self.admission)?;
            return Ok(());
        }
        if ch == '\\' {
            self.admission.work(9.min(tail.len()))?;
            self.at = escape_end(self.css, self.at);
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
                };
                if ch == '{' {
                    self.charge(CssRules, 1)?;
                    self.charge(CssSelectors, 1)?;
                    let head = &self.css[self.statement..self.at];
                    self.admission.work(head.len())?;
                    // Каждый & копирует родительский список; неявный & — один раз.
                    // Неактивные блоки и custom properties также оплачивают эту оценку.
                    let copies = head
                        .bytes()
                        .filter(|&b| b == b'&')
                        .count()
                        .saturating_add(1);
                    let own_chain = selector_depth::measure(head, self.admission)?;
                    let chain = self
                        .chain
                        .checked_mul(copies.saturating_sub(1).max(1))
                        .and_then(|n| n.checked_add(own_chain))
                        .ok_or(Error::CapacityOverflow)?;
                    self.admission.maximum(CssDepth, chain)?;
                    self.counts.0[CssDepth as usize] = self.counts.0[CssDepth as usize].max(chain);
                    self.chain = chain;
                    let expanded = self
                        .expanded
                        .checked_add(6)
                        .and_then(|n| n.checked_mul(copies))
                        .and_then(|n| n.checked_add(head.len()))
                        .ok_or(Error::CapacityOverflow)?;
                    self.charge(CssComponents, expanded)?;
                    self.expanded = expanded;
                    self.statement = self.at + 1;
                }
                self.stack.push(frame, self.admission)?;
            }
            ')' | ']' | '}' if self.stack.last().is_some_and(|f| f.close == ch) => {
                let frame = self.stack.pop().unwrap();
                self.expanded = frame.expanded;
                self.chain = frame.chain;
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
}

#[cfg(test)]
mod tests;
