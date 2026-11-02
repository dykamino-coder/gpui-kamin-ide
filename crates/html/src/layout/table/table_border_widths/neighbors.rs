//! Соседи ячеек по линиям сетки для сросшихся кромок (CSS 2.1 §17.6.2.1): ячейки,
//! чья сторона лежит на данной линии в данной дорожке. Каждая ячейка вносится в
//! каждую дорожку своего охвата, поэтому поиск соседей стоит охвата ячейки, а не
//! числа всех ячеек таблицы.

/// Ячейка сетки: начало и охват по строкам и столбцам (охваты не меньше 1,
/// `slot_grid::TableGrid`).
#[derive(Clone, Copy)]
pub(super) struct Span {
    pub r: usize,
    pub c: usize,
    pub sr: usize,
    pub sc: usize,
}

/// Сторона ячейки.
#[derive(Clone, Copy)]
pub(super) enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Side {
    fn index(self) -> usize {
        self as usize
    }

    fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Right => Self::Left,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
        }
    }
}

impl Span {
    /// Линия сетки, на которой лежит сторона, и дорожки, которые она проходит.
    fn line(self, side: Side) -> (usize, std::ops::Range<usize>) {
        match side {
            Side::Top => (self.r, self.c..self.c + self.sc),
            Side::Bottom => (self.r + self.sr, self.c..self.c + self.sc),
            Side::Left => (self.c, self.r..self.r + self.sr),
            Side::Right => (self.c + self.sc, self.r..self.r + self.sr),
        }
    }
}

/// Стороны всех ячеек по ключу «линия, дорожка», упорядоченные для поиска.
pub(super) struct Lines {
    width: u64,
    sides: [Vec<(u64, u32)>; 4],
}

impl Lines {
    pub(super) fn new(spans: &[Span]) -> Self {
        let width = spans
            .iter()
            .map(|s| (s.r + s.sr).max(s.c + s.sc))
            .max()
            .unwrap_or(0) as u64
            + 1;
        let mut sides: [Vec<(u64, u32)>; 4] = Default::default();
        for side in [Side::Top, Side::Right, Side::Bottom, Side::Left] {
            let list = &mut sides[side.index()];
            for (i, span) in spans.iter().enumerate() {
                let (line, tracks) = span.line(side);
                list.extend(tracks.map(|t| (line as u64 * width + t as u64, i as u32)));
            }
            list.sort_unstable();
        }
        Self { width, sides }
    }

    /// Ячейки, чья противоположная сторона лежит на линии стороны `side` ячейки
    /// `span` и делит с ней хотя бы одну дорожку. Ячейка, делящая несколько
    /// дорожек, встречается по разу на каждую.
    pub(super) fn facing(&self, span: Span, side: Side) -> impl Iterator<Item = usize> + '_ {
        let (line, tracks) = span.line(side);
        let list = &self.sides[side.opposite().index()];
        let width = self.width;
        tracks.flat_map(move |t| {
            let key = line as u64 * width + t as u64;
            let start = list.partition_point(|(k, _)| *k < key);
            list[start..]
                .iter()
                .take_while(move |(k, _)| *k == key)
                .map(|(_, i)| *i as usize)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(r: usize, c: usize, sr: usize, sc: usize) -> Span {
        Span { r, c, sr, sc }
    }

    /// Соседи по индексу совпадают с полным перебором пар (правило §17.6.2.1:
    /// стороны на одной линии и общая дорожка), включая охваты и перекрытия.
    #[test]
    fn facing_cells_equal_the_pairwise_rule() {
        let spans = [
            span(0, 0, 2, 1),
            span(0, 1, 1, 2),
            span(1, 1, 1, 1),
            span(1, 2, 2, 1),
            span(2, 0, 1, 2),
            span(0, 2, 1, 1),
            span(1, 1, 2, 2),
        ];
        let lines = Lines::new(&spans);
        for (i, a) in spans.iter().copied().enumerate() {
            for side in [Side::Top, Side::Right, Side::Bottom, Side::Left] {
                let mut got: Vec<usize> = lines.facing(a, side).filter(|j| *j != i).collect();
                got.sort_unstable();
                got.dedup();
                let want: Vec<usize> = (0..spans.len())
                    .filter(|j| *j != i)
                    .filter(|j| {
                        let b = spans[*j];
                        let cols = a.c < b.c + b.sc && b.c < a.c + a.sc;
                        let rows = a.r < b.r + b.sr && b.r < a.r + a.sr;
                        match side {
                            Side::Top => cols && b.r + b.sr == a.r,
                            Side::Bottom => cols && a.r + a.sr == b.r,
                            Side::Left => rows && b.c + b.sc == a.c,
                            Side::Right => rows && a.c + a.sc == b.c,
                        }
                    })
                    .collect();
                assert_eq!(got, want, "cell {i} side {}", side.index());
            }
        }
    }
}
