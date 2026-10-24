//! Продолжение flex по адресам содержимого; координаты вычисляются после новой раскладки.
use crate::layout::fragment::flow::blocks::Laid;
use crate::tolerance::COORD_EPS;
use std::collections::HashMap;

#[derive(Clone, Debug)]
enum Address {
    Before,
    End,
    Line(usize, usize),
    Box(u64),
    BlockOffset(f32),
}

pub(super) struct Resume {
    items: HashMap<u64, Address>,
}
impl Resume {
    pub(super) fn at(old: &Laid, from: f32) -> Self {
        let items = old
            .flexes
            .values()
            .flat_map(|f| &f.lines)
            .flat_map(|l| &l.items)
            .filter_map(|&id| {
                let (top, bottom) = old.at(id)?;
                let address = if bottom <= from + COORD_EPS {
                    Address::End
                } else if top >= from - COORD_EPS {
                    Address::Before
                } else if let Some(line) = old
                    .content
                    .get(&id)
                    .into_iter()
                    .flatten()
                    .filter(|l| l.bottom > from + COORD_EPS)
                    .min_by(|a, b| a.top.total_cmp(&b.top))
                {
                    Address::Line(line.paragraph, line.byte)
                } else if let Some((&child, _)) = old
                    .descendants
                    .get(&id)
                    .into_iter()
                    .flatten()
                    .filter_map(|child| old.boxes.get_key_value(child))
                    .filter(|(_, p)| p.0 >= from - COORD_EPS && p.1 <= bottom + COORD_EPS)
                    .min_by(|a, b| a.1.0.total_cmp(&b.1.0))
                {
                    Address::Box(child)
                } else {
                    Address::BlockOffset((from - top).max(0.0))
                };
                Some((id, address))
            })
            .collect();
        Self { items }
    }

    fn position(&self, id: u64, new: &Laid) -> Option<f32> {
        let (top, bottom) = new.at(id)?;
        Some(match self.items.get(&id)? {
            Address::Before => top,
            Address::End => bottom,
            Address::Line(paragraph, byte) => {
                new.content
                    .get(&id)?
                    .iter()
                    .find(|l| l.paragraph == *paragraph && l.byte == *byte)?
                    .top
            }
            Address::Box(child) => new.at(*child)?.0,
            Address::BlockOffset(consumed) => top + consumed.min((bottom - top).max(0.0)),
        })
    }

    pub(super) fn consumed(&self, new: &Laid) -> HashMap<u64, (f32, bool)> {
        new.flexes
            .values()
            .filter(|f| !f.column)
            .flat_map(|f| &f.lines)
            .flat_map(|l| &l.items)
            .filter_map(|&id| {
                let (top, bottom) = new.at(id)?;
                let at = self.position(id, new)?;
                Some((
                    id,
                    (
                        (at - top).clamp(0.0, (bottom - top).max(0.0)),
                        matches!(self.items.get(&id), Some(Address::End)),
                    ),
                ))
            })
            .collect()
    }

    pub(super) fn offset(&self, new: &Laid, end: f32) -> f32 {
        new.flexes
            .values()
            .flat_map(|f| {
                f.lines.iter().flat_map(move |line| {
                    line.items.iter().filter_map(move |&id| {
                        if matches!(self.items.get(&id), Some(Address::End) | None) {
                            return None;
                        }
                        if f.column {
                            self.position(id, new)
                        } else {
                            Some(line.start)
                        }
                    })
                })
            })
            .min_by(f32::total_cmp)
            .unwrap_or(end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::flow::ContentLine;
    use crate::layout::fragment::flow::blocks::{FlexFrame, FlexLine};
    #[test]
    fn row_resumes_same_byte_after_width_changes_line_height_and_membership() {
        let mut old = Laid::of_boxes(&[(1, 0., 60.), (2, 0., 20.)]);
        old.flexes.insert(
            9,
            FlexFrame {
                column: false,
                lines: vec![FlexLine {
                    start: 0.,
                    end: 60.,
                    items: vec![1, 2],
                    ..Default::default()
                }],
            },
        );
        old.content.insert(
            1,
            vec![ContentLine {
                state: Default::default(),
                paragraph: 0,
                byte: 8,
                top: 20.,
                bottom: 40.,
            }],
        );
        let token = Resume::at(&old, 20.);
        let mut new = Laid::of_boxes(&[(1, 0., 100.), (2, 100., 120.)]);
        new.flexes.insert(
            9,
            FlexFrame {
                column: false,
                lines: vec![
                    FlexLine {
                        start: 0.,
                        end: 100.,
                        items: vec![1],
                        ..Default::default()
                    },
                    FlexLine {
                        start: 100.,
                        end: 120.,
                        items: vec![2],
                        ..Default::default()
                    },
                ],
            },
        );
        new.content.insert(
            1,
            vec![ContentLine {
                state: Default::default(),
                paragraph: 0,
                byte: 8,
                top: 40.,
                bottom: 60.,
            }],
        );
        assert_eq!(
            token.consumed(&new),
            HashMap::from([(1, (40., false)), (2, (20., true))])
        );
        assert_eq!(token.offset(&new, 120.), 0.);
    }
    #[test]
    fn non_content_box_keeps_consumed_block_size_when_used_height_changes() {
        let old = Laid::of_boxes(&[(1, 0., 60.)]);
        let token = Resume {
            items: HashMap::from([(1, Address::BlockOffset(20.))]),
        };
        let mut new = old;
        new.boxes.insert(1, (0., 100.));
        assert_eq!(token.position(1, &new), Some(20.));
    }
    #[test]
    fn column_resumes_content_instead_of_old_y() {
        let mut old = Laid::of_boxes(&[(1, 0., 60.), (2, 60., 80.)]);
        old.flexes.insert(
            9,
            FlexFrame {
                column: true,
                lines: vec![FlexLine {
                    items: vec![1, 2],
                    ..Default::default()
                }],
            },
        );
        old.content.insert(
            1,
            vec![ContentLine {
                state: Default::default(),
                paragraph: 0,
                byte: 8,
                top: 20.,
                bottom: 40.,
            }],
        );
        let token = Resume::at(&old, 20.);
        let mut new = old.clone();
        new.boxes.insert(1, (0., 100.));
        new.boxes.insert(2, (100., 120.));
        new.content.get_mut(&1).unwrap()[0].top = 40.;
        assert!(token.consumed(&new).is_empty());
        assert_eq!(token.offset(&new, 120.), 40.);
    }
}
