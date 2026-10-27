//! Верхняя оценка рекурсивной цепочки селектора до создания его AST.

use super::*;

pub(super) fn measure(head: &str, admission: &Admission) -> Result<usize, Error> {
    let mut parents = Buffer::default();
    let (mut chain, mut outer, mut maximum) = (0usize, 0usize, 0usize);
    let mut separated = true;
    let mut at = 0;
    while at < head.len() {
        admission.work(1)?;
        let tail = &head[at..];
        let ch = tail.chars().next().unwrap();
        if tail.starts_with("/*") {
            at += 2;
            while at < head.len() && !head[at..].starts_with("*/") {
                admission.work(1)?;
                at += head[at..].chars().next().unwrap().len_utf8();
            }
            at = (at + 2).min(head.len());
            continue;
        }
        match ch {
            ',' => {
                chain = 0;
                separated = true;
            }
            '>' | '+' | '~' => separated = true,
            c if c.is_ascii_whitespace() => separated = true,
            ')' | ']' => {
                if let Some((saved, saved_outer)) = parents.pop() {
                    chain = saved;
                    outer = saved_outer;
                }
                separated = false;
            }
            _ => {
                if separated {
                    chain += 1;
                }
                maximum = maximum.max(outer + chain);
                admission.maximum(CssDepth, maximum)?;
                separated = false;
                match ch {
                    '"' | '\'' => {
                        at += ch.len_utf8();
                        at += try_skip_string(&head[at..], ch, admission)?;
                        continue;
                    }
                    '\\' => {
                        admission.work(9.min(tail.len()))?;
                        at = escape_end(head, at);
                        continue;
                    }
                    '(' | '[' => {
                        parents.push((chain, outer), admission)?;
                        outer += chain;
                        chain = 0;
                        separated = true;
                    }
                    _ => {}
                }
            }
        }
        at += ch.len_utf8();
    }
    Ok(maximum)
}
