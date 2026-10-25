//! Продолжение flex по адресам содержимого; координаты вычисляются после новой раскладки.
use crate::layout::fragment::flow::blocks::Laid;
use crate::tolerance::COORD_EPS;
use std::collections::HashMap;

#[derive(Clone, Debug)]
enum Address {
    Before,
    End,
    // Адрес строки сохраняет расстояние от границы разрыва до её верха:
    // внутренние отбивки ячейки не являются потреблённым содержимым.
    Line(usize, usize, f32),
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
                    // Будущая строка не поглощает остаток монолитной коробки:
                    // до её начала сохраняется физический consumed block-size.
                    .filter(|l| l.top <= from + COORD_EPS && l.bottom > from + COORD_EPS)
                    .min_by(|a, b| a.top.total_cmp(&b.top))
                {
                    Address::Line(line.paragraph, line.byte, from - line.top)
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
            Address::Line(paragraph, byte, before) => {
                new.content
                    .get(&id)?
                    .iter()
                    .find(|l| l.paragraph == *paragraph && l.byte == *byte)?
                    .top
                    + before
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
                // Непочатый item не имеет consumed block-size и не нуждается
                // в ограничении оставшегося содержимого (css-break-3 §5.1).
                if matches!(self.items.get(&id), Some(Address::Before)) {
                    return None;
                }
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

    pub(super) fn offset(&self, new: &Laid) -> Option<f32> {
        new.flexes
            .values()
            .flat_map(|f| {
                f.lines.iter().flat_map(move |line| {
                    line.items.iter().filter_map(move |&id| {
                        if matches!(self.items.get(&id), Some(Address::End) | None) {
                            return None;
                        }
                        if f.column {
                            if matches!(self.items.get(&id), Some(Address::Before)) {
                                // У колонки гасится gap перед новым item, но его
                                // собственное поле остаётся (css-flexbox-1 §10).
                                new.margin_box(id).map(|b| b.0)
                            } else {
                                self.position(id, new)
                            }
                        } else {
                            Some(line.start)
                        }
                    })
                })
            })
            .min_by(f32::total_cmp)
    }
}

#[cfg(test)]
mod tests;
