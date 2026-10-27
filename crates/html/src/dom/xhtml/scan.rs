//! Fallible scans admit work before each inspected byte or fixed comparison.

use crate::{
    budget::{BudgetKind, RevisionBudget},
    document::FailureReason,
};

pub(super) struct Scan<'a>(pub &'a RevisionBudget);

impl Scan<'_> {
    pub fn work(&self, bytes: usize) -> Result<(), FailureReason> {
        self.0
            .consume(BudgetKind::TraversalVisits, bytes as u64)
            .map_err(FailureReason::Budget)
    }

    pub fn byte(&self, text: &str, byte: u8) -> Result<Option<usize>, FailureReason> {
        for (i, b) in text.bytes().enumerate() {
            self.work(1)?;
            if b == byte {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    pub fn literal(&self, text: &str, needle: &str) -> Result<Option<usize>, FailureReason> {
        for (i, b) in text.bytes().enumerate() {
            self.work(1)?;
            if b == needle.as_bytes()[0] {
                self.work(needle.len())?;
                if text.as_bytes()[i..].starts_with(needle.as_bytes()) {
                    return Ok(Some(i));
                }
            }
        }
        Ok(None)
    }

    pub fn name_end(&self, text: &str) -> Result<usize, FailureReason> {
        for (i, c) in text.char_indices() {
            self.work(c.len_utf8())?;
            if c.is_whitespace() || c == '/' || c == '>' {
                return Ok(i);
            }
        }
        Ok(text.len())
    }

    pub fn close(&self, text: &str, needle: &str) -> Result<Option<usize>, FailureReason> {
        for (i, c) in text.char_indices() {
            self.work(c.len_utf8())?;
            if c == '<' {
                self.work(needle.len())?;
                if text[i..]
                    .get(..needle.len())
                    .is_some_and(|t| t.eq_ignore_ascii_case(needle))
                {
                    return Ok(Some(i));
                }
            }
        }
        Ok(None)
    }

    pub fn either(&self, text: &str) -> Result<Option<usize>, FailureReason> {
        for (i, b) in text.bytes().enumerate() {
            self.work(1)?;
            if b == b'&' || b == b'<' {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    pub fn matches(&self, text: &str, choices: &[&str]) -> Result<bool, FailureReason> {
        for choice in choices {
            self.work(
                1 + if text.len() == choice.len() {
                    text.len()
                } else {
                    0
                },
            )?;
            if text.eq_ignore_ascii_case(choice) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
