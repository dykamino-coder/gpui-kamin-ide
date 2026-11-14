//! Перенос множеств по комбинаторам: каждый узел и ребро читаются один раз.

use super::{bits::Bits, has::scope::Scope};
use crate::budget::admission::{Admission, Error};

pub(super) fn project(
    scope: &Scope,
    source: &Bits,
    relation: char,
    backwards: bool,
    admission: &Admission,
) -> Result<Bits, Error> {
    if source.is_empty() {
        return Ok(Bits::empty());
    }
    let mut out = Bits::new(scope.records.len(), admission)?;
    let transitive = matches!(relation, ' ' | '~');
    let neighbour = |i: usize| {
        if matches!(relation, '+' | '~') {
            scope.records[i].previous
        } else {
            scope.records[i].parent
        }
    };
    admission.work(scope.records.len())?;
    if backwards {
        for i in (0..scope.records.len()).rev() {
            if (source.contains(i) || (transitive && out.contains(i)))
                && let Some(to) = neighbour(i)
            {
                out.insert(to);
            }
        }
    } else {
        for i in 0..scope.records.len() {
            if let Some(from) = neighbour(i)
                && (source.contains(from) || (transitive && out.contains(from)))
            {
                out.insert(i);
            }
        }
    }
    Ok(out)
}
