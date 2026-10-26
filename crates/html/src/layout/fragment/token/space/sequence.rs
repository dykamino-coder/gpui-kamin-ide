//! Неизменяемая последовательность внешних фрагментаинеров и адрес вложенного ряда.

use super::{NestedRow, Space};
use crate::layout::fragment::token::Error;
use std::rc::Rc;

/// Владелец разрешает каждый фрагментаинер: размер и идентичность страницы.
#[derive(Clone)]
pub(crate) struct Sequence(Rc<dyn Fn(usize) -> Result<Space, Error>>);

impl std::fmt::Debug for Sequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Sequence")
            .field(&Rc::as_ptr(&self.0))
            .finish()
    }
}

impl Sequence {
    pub(crate) fn new(at: impl Fn(usize) -> Result<Space, Error> + 'static) -> Self {
        Self(Rc::new(at))
    }

    /// Колонки одного разрешённого ряда имеют одинаковые ограничения. Страницы
    /// используют свой поставщик геометрии: :first/named/side меняют размер.
    pub(crate) fn columns(space: Space) -> Self {
        Self::new(move |_| Ok(space))
    }

    pub(crate) fn at(&self, index: usize) -> Result<Space, Error> {
        let space = (self.0)(index)?;
        if space.block_size.is_none_or(|n| !n.is_finite() || n < 0.)
            || space.inline_size.is_some_and(|n| !n.is_finite() || n < 0.)
        {
            return Err(Error::InvalidGeometry);
        }
        Ok(space)
    }

    pub(crate) fn nested(self, start: usize, offset: f32) -> Result<Rc<Nested>, Error> {
        if !offset.is_finite() || offset < 0. {
            return Err(Error::InvalidGeometry);
        }
        self.at(start)?;
        Ok(Rc::new(Nested {
            outer: self,
            start,
            offset,
        }))
    }
}

/// Адресованный вход одного multicol с последовательностью внешних ограничений.
#[derive(Clone, Debug)]
pub(crate) struct Nested {
    pub(crate) outer: Sequence,
    pub(crate) start: usize,
    pub(crate) offset: f32,
}

impl Nested {
    pub(crate) fn outer_at(&self, row: usize) -> Result<Space, Error> {
        self.outer.at(self
            .start
            .checked_add(row)
            .ok_or(Error::ResourceExhausted)?)
    }

    pub(crate) fn at(&self, row: usize) -> Result<Space, Error> {
        let outer = self.outer_at(row)?;
        let offset = if row == 0 { self.offset } else { 0. };
        let nested = NestedRow::from_space(outer, offset).ok_or(Error::InvalidGeometry)?;
        Ok(Space {
            wm: outer.wm,
            inline_size: outer.inline_size,
            min_appeal: outer.min_appeal,
            repeat: outer.repeat,
            not_at_start: usize::from(offset > 0. || !outer.at_start(0)),
            ..Space::nested_row(nested)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::token::space::{Kind, PageIdentity, PageSide};

    #[test]
    fn addressed_rows_use_the_remainder_then_each_actual_outer_space() {
        let sequence = Sequence::new(|index| {
            Ok(Space {
                block_size: Some([400., 100., 60., 180.][index]),
                inline_size: Some(200. + index as f32),
                page: Some(PageIdentity {
                    index: index as u32,
                    name: None,
                    side: PageSide::Right,
                    blank: false,
                }),
                ..Space::pages()
            })
        });
        let nested = sequence.nested(1, 30.).unwrap();
        for (row, block) in [70., 60., 180.].into_iter().enumerate() {
            let space = nested.at(row).unwrap();
            assert_eq!(space.block_size, Some(block));
            assert_eq!(space.inline_size, Some(201. + row as f32));
            assert_eq!(space.at_start(0), row != 0);
            assert!(!space.is_paginated());
            assert!(matches!(space.kind, Kind::NestedRow(r) if r.outer_paged));
            assert_eq!(
                nested.outer_at(row).unwrap().page.unwrap().index as usize,
                row + 1
            );
        }
        let child = Sequence::columns(nested.at(0).unwrap())
            .nested(0, 10.)
            .unwrap();
        assert!(matches!(child.at(0).unwrap().kind, Kind::NestedRow(r) if !r.outer_paged));
        assert_eq!(child.at(0).unwrap().block_size, Some(60.));
    }

    #[test]
    fn nested_space_preserves_enclosing_start_repeat_and_break_appeal() {
        use super::super::{BreakAppeal, RepeatState};
        let outer = Space {
            block_size: Some(90.),
            not_at_start: 1,
            repeat: RepeatState::Inside,
            min_appeal: BreakAppeal::Perfect,
            ..Space::columns()
        };
        let row = Sequence::columns(outer)
            .nested(0, 0.)
            .unwrap()
            .at(0)
            .unwrap();
        assert!(!row.at_start(0));
        assert!(!row.may_break_inside());
        assert_eq!(row.min_appeal, BreakAppeal::Perfect);
        assert_eq!(row, outer.nest_at(0.).unwrap());
    }

    #[test]
    fn invalid_geometry_and_exhaustion_are_explicit_errors() {
        let sequence = Sequence::new(|index| {
            if index == 0 {
                Ok(Space {
                    block_size: Some(100.),
                    ..Space::columns()
                })
            } else {
                Err(Error::ResourceExhausted)
            }
        });
        assert!(matches!(
            sequence.clone().nested(0, f32::NAN),
            Err(Error::InvalidGeometry)
        ));
        let nested = sequence.nested(0, 200.).unwrap();
        assert_eq!(nested.at(0).unwrap().block_size, Some(0.));
        assert_eq!(nested.at(1), Err(Error::ResourceExhausted));
        let invalid = Sequence::columns(Space {
            block_size: Some(f32::INFINITY),
            ..Space::columns()
        });
        assert_eq!(invalid.at(0), Err(Error::InvalidGeometry));
    }
}
