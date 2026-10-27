//! Явный допуск исходных узлов и табличных полос к фрагментам потребителя.

use super::decoration::Decoration;
use gpui::{App, PaintRecordError, PaintSegmentSlice, Window};
use std::{collections::HashMap, rc::Rc};

#[derive(Clone, Default)]
pub(crate) struct SourceSlices {
    pub(crate) nodes: HashMap<u64, Rc<[PaintSegmentSlice]>>,
    pub(crate) boxes: HashMap<u64, Rc<[Decoration]>>,
    pub(crate) slots: HashMap<u64, SourceSlot>,
}

pub(crate) type SourceSlot = Rc<dyn Fn(&mut Window, &mut App) -> Result<(), PaintRecordError>>;
