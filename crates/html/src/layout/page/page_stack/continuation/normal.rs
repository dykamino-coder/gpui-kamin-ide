//! Адрес обычного потока: ребёнок, строка или consumed размер незаполненной части.
use crate::layout::fragment::flow::blocks::{BlockBox, Laid};
use crate::layout::fragment::flow::inline::Inline;
use crate::tolerance::COORD_EPS;

pub(super) enum Cursor {
    Parallel(Vec<Self>),
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
        if let Some(grid) = &blocks.grid {
            // Элементы одного ряда продолжаются независимо (css-break-3 §2.1).
            let cursors: Vec<_> = grid
                .items
                .iter()
                .filter_map(|item| Self::child(item.id, item.inner.as_ref(), old, inline, from))
                .collect();
            if !cursors.is_empty() {
                return Some(Self::Parallel(cursors));
            }
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
        let (top, bottom) = old.at(blocks.id)?;
        if bottom <= from + COORD_EPS {
            return None;
        }
        Some(Self::inside(blocks.id, top, inline, from))
    }

    fn child(
        id: u64,
        inner: Option<&BlockBox>,
        old: &Laid,
        inline: &[Inline],
        from: f32,
    ) -> Option<Self> {
        let (top, bottom) = old.at(id)?;
        if bottom <= from + COORD_EPS {
            None
        } else if top >= from - COORD_EPS {
            Some(Self::Before(id))
        } else if let Some(inner) = inner {
            Self::at(inner, old, inline, from)
        } else {
            Some(Self::inside(id, top, inline, from))
        }
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
        match self {
            Self::Parallel(cursors) => cursors
                .iter()
                .filter_map(|cursor| cursor.offset(new, inline))
                .min_by(f32::total_cmp),
            Self::Before(id) => Some(new.at(*id)?.0),
            Self::Line(paragraph, byte) => inline
                .iter()
                .flat_map(|p| &p.lines)
                .find(|l| l.paragraph == *paragraph && l.state.text_pos == *byte)
                .map(|l| l.top),
            Self::Block(id, consumed) => {
                let (top, bottom) = new.at(*id)?;
                Some(top + consumed.min((bottom - top).max(0.0)))
            }
        }
    }
}

#[cfg(test)]
mod tests;
