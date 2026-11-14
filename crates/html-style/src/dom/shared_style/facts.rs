//! Факты коробки, созданной или перенесённой раскладкой, вне вычисленных значений.

use super::SharedStyle;
use crate::box_facts::BoxContext;
use crate::style::computed::writing_mode::WritingMode;
use crate::style::computed::{Align, BoxStyle};
use std::rc::Rc;

/// Контекст коробки-родителя, стиль элемента, чей каскад пересчитывают варианты
/// `position-try`, коробка родителя по дереву элементов перенесённой коробки и
/// идентичность украшающей коробки стиля, значения которого её не несут.
#[derive(Clone, Debug, Default)]
pub(super) struct BoxFacts {
    pub(super) context: Option<Rc<BoxContext<WritingMode, Align>>>,
    pub(super) try_source: Option<SharedStyle>,
    pub(super) moved_from: Option<Rc<BoxStyle>>,
    pub(super) decor_box: Option<u64>,
}
