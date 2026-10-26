//! Кандидаты класса B для страниц: счёт строк начинается заново в каждом фрагменте.
use crate::layout::fragment::flow::inline::Geometry;
use crate::layout::fragment::token::fc::FcState;
use crate::layout::fragment::token::{Space, Token, TokenStore};
use crate::tolerance::COORD_EPS;

type Address = (Vec<usize>, usize);

/// Арена живёт до завершения ребёнка; новый снимок строк сохраняет адрес токена.
#[derive(Default)]
pub(crate) struct Tokens {
    store: TokenStore,
    next: std::collections::HashMap<Address, crate::layout::fragment::token::TokenId>,
    pending: Vec<(Address, crate::layout::fragment::token::TokenId, f32)>,
}
impl Tokens {
    pub(super) fn commit(&mut self, point: f32) {
        for (paragraph, id, end) in self.pending.drain(..) {
            if (point - end).abs() < COORD_EPS {
                self.next.insert(paragraph, id);
            }
        }
    }
}

type Cut = Option<(f32, f32)>;

pub(super) fn cut(
    tokens: &mut Tokens,
    geometry: &Geometry,
    from: f32,
    room: f32,
    placed: bool,
    shortage: &mut f32,
) -> Result<Option<Cut>, crate::layout::fragment::token::Error> {
    let inline = &geometry.paragraphs;
    let mut edge = from + room;
    if inline.is_empty() {
        return Ok(None);
    }
    loop {
        tokens.pending.clear();
        let mut point = edge;
        for paragraph in inline.iter().filter(|p| {
            p.lines.first().is_some_and(|l| l.top < edge - COORD_EPS)
                && p.lines.last().is_some_and(|l| l.bottom > edge + COORD_EPS)
        }) {
            let Some((end, _)) = paragraph_cut(
                tokens,
                paragraph,
                &geometry.keep,
                from,
                edge,
                placed,
                shortage,
            )?
            else {
                return Ok(Some(None));
            };
            if end > edge + COORD_EPS {
                // Высокая строка занимает несколько физических страниц;
                // адрес IFC не продвигается до последнего среза строки.
                tokens.pending.clear();
                return Ok(None);
            }
            point = point.min(end);
        }
        if let Some(start) = geometry
            .barriers
            .iter()
            .filter(|&&(a, b)| a < point - COORD_EPS && point < b - COORD_EPS)
            .map(|p| p.0)
            .min_by(f32::total_cmp)
        {
            if start <= from + COORD_EPS {
                if placed {
                    return Ok(Some(None));
                }
                // Структурный монолит переполняет физические страницы срезами;
                // его конец не является краем единственного фрагмента IFC.
                // Переполнение неделимой строки обработано выше отдельно.
                return Ok(None);
            }
            point = point.min(start);
        }
        if let Some(start) = geometry
            .keep
            .iter()
            .filter(|&&(a, b)| {
                a < point - COORD_EPS && point < b - COORD_EPS && a >= from - COORD_EPS
            })
            .map(|p| p.0)
            .min_by(f32::total_cmp)
        {
            if start <= from + COORD_EPS {
                if placed {
                    return Ok(Some(None));
                }
            } else {
                point = point.min(start);
            }
        }
        if point >= edge - COORD_EPS {
            return Ok(Some(Some((point, point))));
        }
        // Каждая итерация выбирает более раннюю реальную границу строки.
        edge = point;
        if edge <= from + COORD_EPS {
            return Ok(Some(None));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paragraph_cut(
    tokens: &mut Tokens,
    paragraph: &crate::layout::fragment::flow::inline::Inline,
    keep: &[(f32, f32)],
    from: f32,
    edge: f32,
    placed: bool,
    shortage: &mut f32,
) -> Result<Cut, crate::layout::fragment::token::Error> {
    let Some(start) = paragraph
        .lines
        .iter()
        .position(|l| l.bottom > from + COORD_EPS)
    else {
        return Ok(None);
    };
    let first = &paragraph.lines[start];
    let key = (paragraph.scope.clone(), first.paragraph);
    let previous = tokens.next.get(&key).copied().filter(|id| {
        tokens.store.get(*id).is_some_and(|t| {
            matches!(&t.fc,
            FcState::Inline(state) if state.text_pos == first.state.text_pos)
        })
    });
    let input = match previous {
        Some(id) => id,
        None => {
            // Начальный адрес ещё не произвёл фрагмент IFC.
            let mut token = Token::new(FcState::Inline(first.state.clone()));
            token.seq = 0;
            tokens.store.push(token)?
        }
    };
    let mut space =
        Space::pages().with_not_at_start(usize::from(placed || first.top > from + COORD_EPS));
    space.block_size = Some(edge - from - (first.top - from).max(0.0));
    let avoid = keep.iter().any(|&(a, b)| a < edge && edge < b);
    let candidate = paragraph.resume(space, 0.0, Some(input), &mut tokens.store, avoid)?;
    *shortage = shortage.min(candidate.shortage);
    if candidate.appeal < space.min_appeal {
        return Ok(None);
    }
    if candidate
        .next
        .is_some_and(|id| tokens.store.get(id).is_some_and(|t| t.flags.break_before))
    {
        return Ok((first.top > from + COORD_EPS).then_some((first.top, first.top)));
    }
    if let Some(id) = candidate.next {
        tokens.pending.push((key, id, candidate.end));
    }
    Ok(Some((candidate.end, candidate.end)))
}

#[cfg(test)]
mod tests;
