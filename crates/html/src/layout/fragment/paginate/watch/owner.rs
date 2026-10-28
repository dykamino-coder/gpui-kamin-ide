//! Принадлежность flex-элементов фрагментам копии. Каждая копия раскладывает весь
//! контейнер, а колонка показывает только её окно (вид `slice`, css-break-3 §4). Элемент,
//! чьё начало (начало margin box) лежит после разрыва, принадлежит следующему фрагменту,
//! даже если отрицательное поле вынесло его border box выше начала этого фрагмента: поле
//! после принудительного разрыва сохраняется (css-break-3 §5.2), и Blink рисует такой
//! элемент над своим фрагментаинером (`GiveItemsFinalPositionAndSizeForFragmentation`
//! кладёт элемент в смещение его фрагмента). Копия более раннего фрагмента его не рисует,
//! копия своего фрагмента рисует и его переполнение над окном. Элемент сетки, продолженный
//! со своего расхода, копия сдвигает и обрезает по его окну (`ItemWindow`).
use super::Watched;
use gpui::{Bounds, ContentMask, Pixels, point};
use std::collections::HashMap;
use std::rc::Rc;

/// Как копия рисует наблюдаемую коробку.
pub(super) enum Access {
    /// Коробка не из фрагментов этой копии.
    Hidden,
    /// Свой первый фрагмент: переполнение над окном видно до маски предка.
    Escape(ContentMask<Pixels>),
    Normal,
}

impl Watched {
    /// Рекордер копии фрагмента `part`: те же ограничения роста, что у главной.
    pub(crate) fn for_part(&self, part: usize) -> Rc<Self> {
        let mut copy = self.constraints_only();
        if let Some(c) = Rc::get_mut(&mut copy) {
            c.part = part;
        }
        copy
    }

    /// Первый фрагмент каждого flex-элемента по плану стопки.
    pub(crate) fn set_flex_owners(&self, owners: HashMap<u64, usize>) {
        *self.paint_owners.borrow_mut() = owners;
    }

    /// Окна элементов сеток по копиям (`flow::blocks::grid_items`).
    pub(crate) fn set_item_windows(
        &self,
        windows: HashMap<(usize, u64), crate::layout::fragment::flow::blocks::ItemWindow>,
    ) {
        *self.item_windows.borrow_mut() = windows;
    }

    /// Окно элемента `id` в копии этого рекордера.
    pub(super) fn item_window(
        &self,
        id: u64,
    ) -> Option<crate::layout::fragment::flow::blocks::ItemWindow> {
        self.item_windows.borrow().get(&(self.part, id)).copied()
    }

    /// Маска предка окна фрагмента, заданная перед краской копии.
    pub(crate) fn set_paint_outer(&self, outer: Option<ContentMask<Pixels>>) {
        *self.paint_outer.borrow_mut() = outer;
    }

    pub(super) fn access(&self, id: u64, current: ContentMask<Pixels>) -> Access {
        let Some(&owner) = self.paint_owners.borrow().get(&id) else {
            return Access::Normal;
        };
        if owner > self.part {
            return Access::Hidden;
        }
        match *self.paint_outer.borrow() {
            Some(outer) if owner == self.part && outer.bounds.top() < current.bounds.top() => {
                Access::Escape(ContentMask {
                    bounds: Bounds::from_corners(
                        point(current.bounds.left(), outer.bounds.top()),
                        current.bounds.bottom_right(),
                    ),
                })
            }
            _ => Access::Normal,
        }
    }
}
