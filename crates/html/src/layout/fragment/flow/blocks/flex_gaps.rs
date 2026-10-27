//! Гашение эффективного промежутка ряда flex у разрыва (css-gaps-1 §2, css-flexbox-1 §10):
//! линия, кончившаяся в этом фрагментаинере, отделена от следующей промежутком, который
//! пересекает край; непоглощённая часть промежутка гасится отрицательным ростом линии,
//! и следующая линия начинается у края следующего фрагментаинера.
use super::{BlockBox, FlexPush, Laid};
use crate::tolerance::COORD_EPS;

impl BlockBox {
    /// `(линия перед промежутком, её начало, начало следующей)` для фрагмента
    /// `[from, end]` у края `edge`: разрыв лёг в промежуток после линии
    /// (а не внутрь продолжающегося элемента); принудительные границы рядов
    /// обслуживает их разрыв.
    pub(crate) fn suppressed_row_gaps(
        &self,
        laid: &Laid,
        from: f32,
        end: f32,
        edge: f32,
        out: &mut Vec<(FlexPush, f32, f32)>,
    ) {
        if let Some(flex) = &self.flex
            && let Some(frame) = laid.flexes.get(&self.id)
            && !frame.column
        {
            for pair in frame.lines.windows(2) {
                let (previous, next) = (&pair[0], &pair[1]);
                if !flex.forced_between(previous, next, laid)
                    && previous.end > from + COORD_EPS
                    && previous.end <= end + COORD_EPS
                    && end <= edge + COORD_EPS
                    && next.start > edge + COORD_EPS
                {
                    let line = FlexPush {
                        container: self.id,
                        line: previous.index,
                    };
                    out.push((line, previous.start, next.start));
                }
            }
        }
        for inner in self.nested() {
            inner.suppressed_row_gaps(laid, from, end, edge, out);
        }
    }
}
