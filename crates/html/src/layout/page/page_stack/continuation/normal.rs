//! Адрес обычного потока: ребёнок, строка или consumed размер незаполненной части.
use crate::layout::fragment::flow::blocks::{BlockBox, Laid};
use crate::layout::fragment::flow::inline::Inline;
use crate::tolerance::COORD_EPS;

pub(super) enum Cursor {
    Before(u64),
    Line(usize, usize),
    Block(u64, f32),
}

impl Cursor {
    pub(super) fn at(blocks: &BlockBox, old: &Laid, inline: &[Inline], from: f32) -> Option<Self> {
        // Независимые flex-потоки продолжаются собственными адресами элементов.
        if blocks.flex.is_some() {
            return None;
        }
        for child in &blocks.kids {
            let Some((top, bottom)) = old.at(child.id) else {
                continue;
            };
            if bottom <= from + COORD_EPS {
                continue;
            }
            if top >= from - COORD_EPS {
                return Some(Self::Before(child.id));
            }
            if let Some(inner) = &child.inner {
                return Self::at(inner, old, inline, from);
            }
            return Some(Self::inside(child.id, top, inline, from));
        }
        let (top, _) = old.at(blocks.id)?;
        Some(Self::inside(blocks.id, top, inline, from))
    }

    fn inside(id: u64, top: f32, inline: &[Inline], from: f32) -> Self {
        if let Some(line) = inline
            .iter()
            .flat_map(|p| &p.lines)
            .find(|l| l.top <= from + COORD_EPS && l.bottom > from + COORD_EPS)
        {
            Self::Line(line.paragraph, line.state.text_pos)
        } else {
            // Оставшаяся definite/min-height существует и после последней строки.
            Self::Block(id, (from - top).max(0.0))
        }
    }

    pub(super) fn offset(&self, new: &Laid, inline: &[Inline]) -> Option<f32> {
        match *self {
            Self::Before(id) => Some(new.at(id)?.0),
            Self::Line(paragraph, byte) => inline
                .iter()
                .flat_map(|p| &p.lines)
                .find(|l| l.paragraph == paragraph && l.state.text_pos == byte)
                .map(|l| l.top),
            Self::Block(id, consumed) => {
                let (top, bottom) = new.at(id)?;
                Some(top + consumed.min((bottom - top).max(0.0)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::flow::ContentLine;
    use crate::layout::fragment::flow::blocks::{BlockKid, FlexBox};

    #[test]
    fn sibling_boundary_tracks_the_child_after_relayout() {
        let blocks = BlockBox {
            id: 1,
            kids: vec![
                BlockKid {
                    id: 2,
                    ..Default::default()
                },
                BlockKid {
                    id: 3,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let old = Laid::of_boxes(&[(1, 0., 288.), (2, 0., 96.), (3, 96., 288.)]);
        let cursor = Cursor::at(&blocks, &old, &[], 96.).unwrap();
        let new = Laid::of_boxes(&[(1, 0., 320.), (2, 0., 128.), (3, 128., 320.)]);
        assert_eq!(cursor.offset(&new, &[]), Some(128.));
    }

    #[test]
    fn empty_remainder_keeps_consumed_size_instead_of_jumping_to_the_next_child() {
        let blocks = BlockBox {
            id: 1,
            kids: vec![
                BlockKid {
                    id: 2,
                    ..Default::default()
                },
                BlockKid {
                    id: 3,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let old = Laid::of_boxes(&[(1, 0., 300.), (2, 0., 200.), (3, 200., 300.)]);
        let cursor = Cursor::at(&blocks, &old, &[], 96.).unwrap();
        let new = Laid::of_boxes(&[(1, 0., 400.), (2, 0., 300.), (3, 300., 400.)]);
        assert_eq!(cursor.offset(&new, &[]), Some(96.));
    }

    #[test]
    fn definite_height_continues_after_its_last_line() {
        let blocks = BlockBox {
            id: 1,
            ..Default::default()
        };
        let old = Laid::of_boxes(&[(1, 0., 600.)]);
        let inline = vec![Inline {
            lines: vec![ContentLine {
                top: 0.,
                bottom: 20.,
                ..Default::default()
            }],
            ..Default::default()
        }];
        let cursor = Cursor::at(&blocks, &old, &inline, 200.).unwrap();
        assert_eq!(cursor.offset(&old, &inline), Some(200.));
    }

    #[test]
    fn normal_line_uses_content_address_after_rewrap() {
        let blocks = BlockBox {
            id: 1,
            ..Default::default()
        };
        let old = Laid::of_boxes(&[(1, 0., 100.)]);
        let mut line = ContentLine {
            paragraph: 2,
            top: 20.,
            bottom: 40.,
            ..Default::default()
        };
        line.state.text_pos = 12;
        let mut inline = vec![Inline {
            lines: vec![line],
            ..Default::default()
        }];
        let cursor = Cursor::at(&blocks, &old, &inline, 20.).unwrap();
        inline[0].lines[0].top = 60.;
        assert_eq!(cursor.offset(&old, &inline), Some(60.));
    }

    #[test]
    fn nested_flex_keeps_its_independent_item_cursors() {
        let blocks = BlockBox {
            id: 1,
            kids: vec![BlockKid {
                id: 2,
                inner: Some(BlockBox {
                    id: 2,
                    flex: Some(FlexBox::default()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let old = Laid::of_boxes(&[(1, 0., 100.), (2, 0., 100.)]);
        assert!(Cursor::at(&blocks, &old, &[], 50.).is_none());
    }
}
