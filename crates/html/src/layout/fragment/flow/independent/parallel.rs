//! Продолжения детей за normal block-end; адреса не зависят от LayoutId копии.

use crate::layout::fragment::flow::blocks::{BlockBox, Laid};
use crate::layout::fragment::token::cursor::ChildCursor;
use crate::layout::fragment::token::fc::{BlockState, ChildToken, FcState};
use crate::layout::fragment::token::{Error, Token, TokenId, TokenStore};
use crate::tolerance::COORD_EPS;
use std::collections::HashMap;
mod children;
mod position;
mod progress;
mod record;
use children::children;

/// Протяжённость видимого потока не меняет normal border box контейнера.
pub(crate) fn flow_end(b: &BlockBox, laid: &Laid) -> Option<f32> {
    let (_, mut end) = laid.at(b.id)?;
    let mut stack = vec![b];
    while let Some(b) = stack.pop() {
        let Some((_, bottom)) = laid.at(b.id) else {
            continue;
        };
        end = end.max(bottom);
        if b.clips_block {
            continue;
        }
        for child in children(b) {
            if let Some(inner) = child.inner
                && laid.at(inner.id).is_some()
            {
                stack.push(inner);
            } else if let Some((_, bottom)) = laid.at(child.id) {
                end = end.max(bottom);
            }
        }
        end = laid
            .flows
            .get(&b.id)
            .into_iter()
            .flat_map(|f| &f.solid)
            .map(|p| p.1)
            .fold(end, f32::max);
    }
    Some(end)
}

#[derive(Debug)]
pub(crate) struct Parallel {
    store: TokenStore,
    ids: HashMap<u64, TokenId>,
    root: Option<TokenId>,
}

impl Parallel {
    pub(crate) fn at(
        b: &BlockBox,
        laid: &Laid,
        from: f32,
        previous: Option<(&Self, bool)>,
    ) -> Result<Self, Error> {
        let mut result = Self {
            store: TokenStore::default(),
            ids: HashMap::new(),
            root: None,
        };
        result.root = result.record(b, laid, from, previous)?;
        Ok(result)
    }

    /// Только потомки, оставшиеся после normal конца предка. Обычный
    /// курсор и строчные tokens продолжаются своими алгоритмами.
    pub(crate) fn offset(&self, b: &BlockBox, new: &Laid) -> Option<f32> {
        self.root
            .and_then(|root| self.position(root, b, new, false))
    }
}

#[cfg(test)]
mod tests;
