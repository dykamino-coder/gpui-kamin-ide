//! Ряды текстовых колонок в ограничениях реальных внешних фрагментаинеров.

use crate::layout::fragment::token::{Error, space::sequence::Nested};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    /// Диапазон фрагментов колонок, независимый от числа байтов и строк.
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) y: f32,
    pub(super) block: f32,
    pub(super) lines: usize,
}

#[derive(Default)]
pub(super) struct Partition {
    pub(super) ends: Vec<usize>,
    pub(super) rows: Vec<Row>,
    pub(super) height: f32,
}

pub(super) fn partition(
    lines: usize,
    count: usize,
    line: f32,
    outer: &Nested,
    balance: bool,
    (orphans, widows): (usize, usize),
) -> Result<Partition, Error> {
    if count == 0 || !line.is_finite() || line <= 0. {
        return Err(Error::InvalidGeometry);
    }
    let mut plan = Partition::default();
    let (mut used, mut row, mut y) = (0, 0usize, 0.);
    while used < lines {
        let space = outer.at(row)?;
        let cap = space.block_size.ok_or(Error::InvalidGeometry)?;
        let per = (cap / line).floor() as usize;
        // Блок переносится из частичного фрагментаинера перед ослаблением
        // orphans или переполнением неделимой строкой (css-break-3 §4.4).
        if per < orphans && !space.at_start(0) && lines - used > per {
            y += cap;
            if !y.is_finite() {
                return Err(Error::InvalidGeometry);
            }
            row = row.checked_add(1).ok_or(Error::ResourceExhausted)?;
            continue;
        }
        if cap == 0. {
            return Err(Error::NoProgress);
        }
        let per = per.max(1);
        let remaining = lines - used;
        let balanced = if balance && remaining.div_ceil(count) <= per {
            super::balance::last_row(remaining, count, per, used > 0, (orphans, widows))?
        } else {
            None
        };
        let start = plan.ends.len();
        let mut tallest = 0;
        for column in 0..count {
            if used == lines {
                break;
            }
            if balanced.as_ref().is_some_and(|v| column == v.len()) {
                break;
            }
            let mut take = balanced
                .as_ref()
                .map_or(per, |v| v[column])
                .min(lines - used);
            let tail = lines - used - take;
            if tail > 0 && tail < widows {
                let before = (lines - used).saturating_sub(widows);
                if before >= orphans && before < take {
                    take = before;
                }
            }
            used += take;
            tallest = tallest.max(take);
            plan.ends
                .try_reserve(1)
                .map_err(|_| Error::ResourceExhausted)?;
            plan.ends.push(used);
        }
        let block = if used == lines {
            tallest as f32 * line
        } else {
            cap
        };
        plan.rows
            .try_reserve(1)
            .map_err(|_| Error::ResourceExhausted)?;
        plan.rows.push(Row {
            start,
            end: plan.ends.len(),
            y,
            block,
            lines: tallest,
        });
        plan.height = y + block;
        y += cap;
        if !y.is_finite() || !plan.height.is_finite() {
            return Err(Error::InvalidGeometry);
        }
        row = row.checked_add(1).ok_or(Error::ResourceExhausted)?;
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::token::{Space, space::sequence::Sequence};

    fn outer(blocks: &'static [f32], offset: f32) -> std::rc::Rc<Nested> {
        Sequence::new(move |index| {
            blocks
                .get(index)
                .copied()
                .map(|block| Space {
                    block_size: Some(block),
                    ..Space::pages()
                })
                .ok_or(Error::ResourceExhausted)
        })
        .nested(0, offset)
        .unwrap()
    }

    #[test]
    fn first_partial_and_varying_rows_consume_every_line_once() {
        let plan = partition(23, 2, 10., &outer(&[100., 30., 80.], 60.), true, (1, 1)).unwrap();
        assert_eq!(plan.ends, [4, 8, 11, 14, 19, 23]);
        assert_eq!(
            plan.rows,
            [
                Row {
                    start: 0,
                    end: 2,
                    y: 0.,
                    block: 40.,
                    lines: 4
                },
                Row {
                    start: 2,
                    end: 4,
                    y: 40.,
                    block: 30.,
                    lines: 3
                },
                Row {
                    start: 4,
                    end: 6,
                    y: 70.,
                    block: 50.,
                    lines: 5
                },
            ]
        );
        assert_eq!(plan.height, 120.);
    }

    #[test]
    fn partial_row_moves_before_relaxing_orphans() {
        let plan = partition(6, 2, 10., &outer(&[100., 40.], 90.), true, (2, 2)).unwrap();
        assert_eq!(plan.ends, [3, 6]);
        assert_eq!(plan.rows[0].y, 10.);
        assert_eq!(plan.height, 40.);
    }

    #[test]
    fn zero_remainder_moves_to_next_outer_fragmentainer() {
        let plan = partition(4, 2, 10., &outer(&[100., 20.], 100.), true, (1, 1)).unwrap();
        assert_eq!(plan.ends, [2, 4]);
        assert_eq!(plan.height, 20.);
    }

    #[test]
    fn indivisible_line_overflows_a_short_fragmentainer() {
        let plan = partition(2, 2, 10., &outer(&[5.], 0.), true, (2, 2)).unwrap();
        assert_eq!(plan.ends, [1, 2]);
        assert_eq!(plan.height, 10.);
    }

    #[test]
    fn exhausted_sequence_does_not_repeat_the_previous_space() {
        assert!(matches!(
            partition(8, 2, 10., &outer(&[20.], 0.), false, (1, 1)),
            Err(Error::ResourceExhausted)
        ));
    }

    #[test]
    fn last_row_uses_fewer_columns_before_violating_line_minima() {
        let plan = partition(3, 2, 10., &outer(&[100.], 0.), true, (2, 2)).unwrap();
        assert_eq!(plan.ends, [3]);
        assert_eq!(plan.height, 30.);
        let plan = partition(10, 4, 10., &outer(&[100.], 0.), true, (2, 4)).unwrap();
        assert_eq!(plan.ends, [2, 6, 10]);
        assert_eq!(plan.rows[0].lines, 4);
    }
}
