//! Восстановление поперечных краёв растянутых строк гибкого контейнера (`align-content:
//! stretch`) по началам строк и полю содержимого: конец строки доводится до начала следующей
//! минус зазор; выравнивание по центру/концу и переполнение оставляют измеренную геометрию.

use super::GAP_RULE_EPS;

pub(super) fn stretch(lines: &mut [(f32, f32)], gap: f32, start: f32, end: f32) {
    let Some(first) = lines.first() else { return };
    // Center/end alignment or overflowing content cannot be reconstructed
    // from item starts. Keep its measured geometry instead.
    if (first.0 - start).abs() > GAP_RULE_EPS
        || lines.last().is_some_and(|l| l.1 > end + GAP_RULE_EPS)
    {
        return;
    }
    for k in 0..lines.len() {
        let next = lines.get(k + 1).map_or(end, |l| l.0 - gap);
        lines[k].1 = lines[k].1.max(next);
    }
}
