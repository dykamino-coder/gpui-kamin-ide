//! Разрыв одного IFC по окончательным line boxes; координаты в логической блочной оси.
mod cursor;
pub(crate) use cursor::Cursor;

use crate::layout::fragment::token::fc::{FcState, InlineState};
use crate::layout::fragment::token::space::BreakAppeal;
use crate::layout::fragment::token::{Error, Space, Token, TokenId, TokenStore};
use crate::tolerance::COORD_EPS;

#[derive(Clone, Debug, Default)]
pub(crate) struct Inline {
    pub(crate) lines: Vec<super::ContentLine>,
    pub(crate) orphans: usize,
    pub(crate) widows: usize,
}

/// Строки и структурные запреты одного измеренного снимка.
#[derive(Clone, Debug, Default)]
pub(crate) struct Geometry {
    pub(crate) paragraphs: Vec<Inline>,
    pub(crate) barriers: Vec<(f32, f32)>,
    pub(crate) keep: Vec<(f32, f32)>,
}

/// Кандидат возвращает точный адрес следующей строки, а не оценку высоты текста.
#[derive(Debug)]
pub(crate) struct Candidate {
    pub(crate) end: f32,
    pub(crate) next: Option<TokenId>,
    pub(crate) appeal: BreakAppeal,
    pub(crate) shortage: f32,
}

impl Inline {
    pub(crate) fn translated(&self, offset: f32) -> Self {
        let mut result = self.clone();
        for line in &mut result.lines {
            line.top += offset;
            line.bottom += offset;
        }
        result
    }

    /// CSS Break 3 §4.4: сначала ослабляются widows/orphans, затем avoid.
    /// При одинаковой привлекательности выбирается последняя помещающаяся строка.
    pub(crate) fn resume(
        &self,
        space: Space,
        offset: f32,
        input: Option<TokenId>,
        store: &mut TokenStore,
        avoid: bool,
    ) -> Result<Candidate, Error> {
        let room = space.block_size.ok_or(Error::InvalidGeometry)? - offset;
        if !room.is_finite() || !offset.is_finite() || offset < 0.0 {
            return Err(Error::InvalidGeometry);
        }
        let nonempty = offset > COORD_EPS || !space.at_start(0);
        let previous = input
            .map(|id| store.get(id).ok_or(Error::InvalidToken))
            .transpose()?;
        let state = match previous.map(|t| &t.fc) {
            Some(FcState::Inline(state)) if state.child.is_none() => state.clone(),
            None => InlineState::default(),
            _ => return Err(Error::InvalidToken),
        };
        let start = if input.is_some() {
            self.lines
                .iter()
                .position(|l| l.state.text_pos == state.text_pos)
                .ok_or(Error::InvalidToken)?
        } else {
            0
        };
        let lines = &self.lines[start..];
        let Some(first) = lines.first() else {
            return Ok(Candidate {
                end: 0.0,
                next: None,
                appeal: BreakAppeal::Perfect,
                shortage: 0.0,
            });
        };
        if lines
            .iter()
            .any(|l| !l.top.is_finite() || !l.bottom.is_finite() || l.bottom < l.top)
            || lines.windows(2).any(|p| p[0].bottom > p[1].top + COORD_EPS)
        {
            return Err(Error::InvalidGeometry);
        }
        let origin = first.top;
        let fitting = lines
            .iter()
            .take_while(|l| l.bottom - origin <= room + COORD_EPS)
            .count();
        if fitting == lines.len() {
            return Ok(Candidate {
                end: lines.last().unwrap().bottom,
                next: None,
                appeal: BreakAppeal::Perfect,
                shortage: 0.0,
            });
        }
        let minimum = if state.past_first_line {
            self.orphans.max(self.widows)
        } else {
            self.orphans
        }
        .max(1);
        let appeal_at = |count: usize| {
            if avoid {
                BreakAppeal::ViolatingAvoid
            } else if count < minimum || lines.len() - count < self.widows.max(1) {
                BreakAppeal::ViolatingOrphansWidows
            } else {
                BreakAppeal::Perfect
            }
        };
        let best = (1..=fitting).map(|n| (appeal_at(n), n)).max();
        // Непустой фрагментаинер может сохранить все ограничения переносом IFC.
        let count = match best {
            Some((appeal, n))
                if appeal >= space.min_appeal && (!nonempty || appeal == BreakAppeal::Perfect) =>
            {
                n
            }
            _ if nonempty => 0,
            _ => fitting.max(1), // Высокая строка неделима и переполняет пустой фрагмент.
        };
        let end = if count == 0 {
            origin
        } else {
            lines[count - 1].bottom + lines.get(count).map_or(0.0, |line| line.consumed_before)
        };
        let shortage = (lines[fitting].bottom - origin - room).max(0.0);
        if count == lines.len() {
            return Ok(Candidate {
                end,
                next: None,
                appeal: BreakAppeal::LastResort,
                shortage,
            });
        }
        let fc = FcState::Inline(lines[count].state.clone());
        let mut token = if count == 0 {
            previous.cloned().unwrap_or_else(|| {
                let mut t = Token::new(fc.clone());
                t.seq = 0;
                t
            })
        } else if let Some(previous) = previous {
            previous.advance(end - origin, fc.clone())?
        } else {
            let mut t = Token::new(fc.clone());
            t.consumed = end - origin;
            t
        };
        token.fc = fc;
        token.flags.break_before = count == 0;
        let next = Some(store.push(token)?);
        Ok(Candidate {
            end,
            next,
            appeal: if count == 0 {
                BreakAppeal::Perfect
            } else {
                appeal_at(count)
            },
            shortage,
        })
    }
}

#[cfg(test)]
mod tests;
