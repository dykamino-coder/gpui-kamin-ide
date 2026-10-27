//! Очередь независимых потоков: abspos продлевает план, fixed повторяет готовые страницы.

use super::{Child, OofResume, resume};
use crate::layout::fragment::token::{Error, Space, TokenId, TokenStore, space::RepeatState};

pub(crate) struct Batch<T> {
    pub(crate) fragments: Vec<(u32, T)>,
    pub(crate) next: Vec<OofResume>,
    pub(crate) needs_space: bool,
}

/// Владельцы idx/CB/static anchor не меняются между вызовами. Сначала узнаём,
/// остались ли независимые abspos, затем завершаем fixed на последней странице.
pub(crate) fn resume_all<T>(
    pending: &[OofResume],
    space: Space,
    normal_complete: bool,
    store: &mut TokenStore,
    space_for: impl Fn(u32, Space) -> Space,
    mut child: impl FnMut(u32, f32, Option<TokenId>, Space, &mut TokenStore) -> Result<Child<T>, Error>,
) -> Result<Batch<T>, Error> {
    let mut fragments = Vec::new();
    let mut next = Vec::new();
    let mut needs_space = false;
    for repeated in [false, true] {
        let current = if repeated {
            Space {
                repeat: RepeatState::Repeat {
                    last: normal_complete && !needs_space,
                },
                ..space
            }
        } else {
            space
        };
        for (order, &input) in pending
            .iter()
            .enumerate()
            .filter(|(_, input)| input.repeat_fixed == repeated)
        {
            let current = space_for(input.idx, current);
            let output = resume(input, current, store, |start, token, store| {
                child(input.idx, start, token, current, store)
            })?;
            needs_space |= output.needs_space();
            if let Some(fragment) = output.fragment {
                fragments.push((order, input.idx, fragment));
            }
            if let Some(input) = output.next {
                next.push((order, input));
            }
        }
    }
    fragments.sort_by_key(|(order, _, _)| *order);
    next.sort_by_key(|(order, _)| *order);
    Ok(Batch {
        fragments: fragments
            .into_iter()
            .map(|(_, idx, fragment)| (idx, fragment))
            .collect(),
        next: next.into_iter().map(|(_, input)| input).collect(),
        needs_space,
    })
}

#[cfg(test)]
mod tests;
