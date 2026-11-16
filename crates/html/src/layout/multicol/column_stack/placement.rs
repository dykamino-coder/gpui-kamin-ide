//! Место колонки стопки: линия колонок и номер в ней.

use super::ColumnStack;

impl ColumnStack {
    /// Колонка `col` стоит в линии `col / count`: ряды с переносом либо линии
    /// между спаннерами без переполняющих колонок (`discard`).
    pub(crate) fn wraps(&self) -> bool {
        self.rows.is_some_and(|r| r.wrap) || self.discard
    }

    /// Где стоит колонка `col`: номер в линии и смещение линии. Без рядов и
    /// при `nowrap` колонки идут вбок сплошь (переполняющие — за край).
    pub(crate) fn place(&self, col: usize) -> (usize, f32) {
        if self.wraps() {
            let lines = self.lines_plan.borrow();
            (
                col % self.count,
                lines.get(col / self.count).map_or(0.0, |l| l.0),
            )
        } else {
            (col, 0.0)
        }
    }
}
