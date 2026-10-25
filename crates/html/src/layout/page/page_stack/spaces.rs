//! Пространства реальных листов: имена интернируются в пределах одной стопки.

use super::{PageGeom, PageGeomFn, PageStack};
use crate::layout::fragment::token::Space;
use crate::layout::fragment::token::space::{PageIdentity, PageSide};
use crate::style::computed::writing_mode::WritingMode;

impl PageStack {
    pub(crate) fn with_principal(mut self, principal: WritingMode) -> Self {
        self.principal = principal;
        self
    }
}

pub(super) struct Pages<'a> {
    pub(super) names: &'a [String],
    pub(super) interned: &'a [String],
    pub(super) geoms: &'a [PageGeom],
    pub(super) geom_for: &'a PageGeomFn,
    pub(super) principal: WritingMode,
}

impl Pages<'_> {
    pub(super) fn at(&self, index: usize) -> Space {
        let name = self
            .names
            .get(index)
            .or_else(|| self.names.last())
            .map_or("", String::as_str);
        let geom = self
            .geoms
            .get(index)
            .copied()
            .unwrap_or_else(|| (self.geom_for)(index, name));
        let (inline, block) = if self.principal.vertical() {
            (geom.area.1, geom.area.0)
        } else {
            geom.area
        };
        let index = u32::try_from(index).expect("allocated page index fits u32");
        Space {
            wm: self.principal,
            inline_size: Some(inline.max(1.0)),
            block_size: Some(block.max(1.0)),
            page: Some(PageIdentity {
                index,
                name: (!name.is_empty()).then(|| {
                    self.interned
                        .iter()
                        .position(|n| n == name)
                        .and_then(|n| u32::try_from(n).ok())
                        .expect("interned page name")
                }),
                side: PageSide::of(index, self.principal),
                // Этот consumer создаёт листы с normal flow; вставляемые
                // стороны left/right принадлежат обработке forced page breaks.
                blank: false,
            }),
            ..Space::pages()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::{Computed, writing_mode::WritingModeKw};
    use std::rc::Rc;

    #[test]
    fn new_sheets_use_their_index_name_and_principal_axes() {
        let geom_for: PageGeomFn = Rc::new(|index, name| {
            let area = (
                200.0 + index as f32,
                if name == "chapter" { 300.0 } else { 100.0 },
            );
            PageGeom {
                size: area,
                area,
                margin: [0.0; 4],
                padding: [0.0; 4],
                border: (0.0, gpui::black()),
                outline: (0.0, 0.0, gpui::black()),
                bg: gpui::white(),
                canvas: None,
                turn: 0,
            }
        });
        let names = [String::new(), "chapter".into()];
        let interned = ["chapter".into()];
        let geoms = [geom_for(0, ""), geom_for(1, "chapter")];
        let mut style = Computed::default();
        style.writing_mode = Some(WritingModeKw::VerticalRl);
        let pages = Pages {
            names: &names,
            interned: &interned,
            geoms: &geoms,
            geom_for: &geom_for,
            principal: style.wm(),
        };
        assert_eq!(pages.at(0).page.unwrap().name, None);
        let space = pages.at(2);
        assert_eq!(
            (space.inline_size, space.block_size),
            (Some(300.0), Some(202.0))
        );
        assert_eq!(
            space.page,
            Some(PageIdentity {
                index: 2,
                name: Some(0),
                side: PageSide::Left,
                blank: false,
            })
        );
        assert_eq!(pages.at(1).page.unwrap().name, space.page.unwrap().name);
    }
}
