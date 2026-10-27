//! Данные подложек текста и выбор ближайшей в координатах текущего paint.

use gpui::{Bounds, Pixels, Point, TransformationMatrix, Window};
use std::rc::Rc;

/// Фон строки/группы таблицы перенесён на ячейку только для рисования.
pub(crate) struct ReplicatedBackground;

pub(crate) struct Background {
    pub bounds: Bounds<Pixels>,
    pub offset: Point<Pixels>,
    pub transform: TransformationMatrix,
    pub parent: Option<Rc<Self>>,
}

/// Прозрачный потомок использует содержащую подложку в той же системе координат.
pub(crate) fn background_offset(window: &Window, baseline: Point<Pixels>) -> Option<Point<Pixels>> {
    let transform = window.current_transformation();
    let mut background = window.raster_context::<Background>();
    while let Some(frame) = background {
        if frame.transform == transform && frame.bounds.contains(&baseline) {
            return Some(frame.offset);
        }
        background = frame.parent.clone();
    }
    None
}

impl Drop for Background {
    fn drop(&mut self) {
        // Снимок может пережить всех предков: освобождаем уникальный хвост
        // итеративно, независимо от глубины вложенных фоновых коробок.
        let mut parent = self.parent.take();
        while let Some(frame) = parent {
            let Ok(mut frame) = Rc::try_unwrap(frame) else {
                break;
            };
            parent = frame.parent.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_retained_background_chain_releases_ancestors_without_recursion() {
        let make = |parent| {
            Rc::new(Background {
                bounds: Bounds::default(),
                offset: Point::default(),
                transform: TransformationMatrix::unit(),
                parent,
            })
        };
        let mut leaf = make(None);
        let root = Rc::downgrade(&leaf);
        for _ in 0..10_000 {
            leaf = make(Some(leaf));
        }
        assert!(root.upgrade().is_some());
        drop(leaf);
        assert!(root.upgrade().is_none());
    }
}
