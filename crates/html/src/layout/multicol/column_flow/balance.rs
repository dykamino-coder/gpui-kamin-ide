//! Баланс текстовых колонок последнего ряда с ограниченным внешним бюджетом.

use crate::layout::fragment::token::Error;

pub(super) fn last_row(
    lines: usize,
    count: usize,
    capacity: usize,
    continued: bool,
    (orphans, widows): (usize, usize),
) -> Result<Option<Vec<usize>>, Error> {
    let middle = orphans.max(widows).max(1);
    let first = if continued { middle } else { orphans.max(1) };
    // Промежуточная колонка соблюдает оба минимума. Число колонок уменьшается
    // до ослабления ограничений строк (css-break-3 §4.4).
    let max_columns = if lines >= first.saturating_add(widows) {
        2 + (lines - first - widows) / middle
    } else {
        1
    };
    let columns = count.min(max_columns).min(lines).max(1);
    let target = if columns == 1 {
        lines
    } else {
        lines.div_ceil(columns).max(first).max(widows)
    };
    if target > capacity {
        return Ok(None);
    }
    let mut takes = Vec::new();
    takes
        .try_reserve(columns)
        .map_err(|_| Error::ResourceExhausted)?;
    let mut left = lines;
    for index in 0..columns {
        let after = columns - index - 1;
        let take = if after == 0 {
            left
        } else {
            let minimum = if index == 0 { first } else { middle };
            let reserve = widows + (after - 1) * middle;
            left.div_ceil(after + 1).max(minimum).min(left - reserve)
        };
        takes.push(take);
        left -= take;
    }
    Ok(Some(takes))
}
