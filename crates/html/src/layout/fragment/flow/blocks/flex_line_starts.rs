//! Принудительный разрыв перед первым элементом линии flex-колонки (css-flexbox-1 §10):
//! «forced breaks on other items are applied to the item itself». Последовательный поток
//! линии ставит разрывы только между соседями, а у первого элемента линии соседа нет; если он
//! не первый элемент контейнера (тогда разрыв переходит на контейнер) и отделён от края
//! контейнера, его собственный разрыв переносит элемент вместе с полем в следующий
//! фрагментаинер независимо от параллельных линий.
use super::{BlockBox, Laid};
use crate::tolerance::COORD_EPS;

impl BlockBox {
    /// `(элемент, начало его margin box)` для первых элементов линий колонок
    /// с принудительным разрывом перед ними.
    pub(crate) fn forced_line_starts(&self, laid: &Laid, out: &mut Vec<(u64, f32)>) {
        if let Some(flex) = &self.flex
            && let Some(frame) = laid.flexes.get(&self.id)
            && frame.column
        {
            let first = frame
                .lines
                .iter()
                .flat_map(|line| &line.order)
                .min_by_key(|entry| entry.0)
                .and_then(|entry| entry.1);
            for line in &frame.lines {
                let start = |id: &u64| laid.margin_box(*id).or_else(|| laid.at(*id)).map(|m| m.0);
                let Some(head) = line
                    .items
                    .iter()
                    .filter_map(|id| Some((*id, start(id)?)))
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                else {
                    continue;
                };
                // Класс A перед элементом есть, только если его отделяет от края
                // контейнера поле, отбивка или рамка (Blink
                // `has_container_separation`: смещение border box > 0); иначе
                // граница — край контейнера, и разрыв переходит на него.
                let separated = laid
                    .at(head.0)
                    .zip(laid.at(self.id))
                    .is_some_and(|(item, container)| item.0 > container.0 + COORD_EPS);
                if separated
                    && Some(head.0) != first
                    && flex
                        .items
                        .iter()
                        .any(|item| item.id == head.0 && item.edge(false, laid).0)
                {
                    out.push(head);
                }
            }
        }
        for inner in self.nested() {
            inner.forced_line_starts(laid, out);
        }
    }
}

/// Разрыв перед первым элементом линии в этом фрагменте ещё не случился: элемент
/// внутри фрагмента либо в самом начале первого фрагмента копии (`from` = 0),
/// где ему предшествует содержимое других линий, а не граница фрагментаинера.
pub(crate) fn pending(start: f32, from: f32, edge: f32) -> bool {
    start < edge - COORD_EPS
        && (start > from + COORD_EPS || (from.abs() <= COORD_EPS && start >= -COORD_EPS))
}
