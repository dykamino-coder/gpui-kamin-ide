//! Продвижение содержимого: смена seq, полей и exclusion-геометрии не считается работой.

use crate::layout::fragment::token::{
    Token, TokenId, TokenStore,
    fc::{ChildToken, FcState},
};
use std::collections::HashSet;

pub(super) fn advances(before: &Token, after: &Token, store: &TokenStore) -> bool {
    let mut pending = vec![(before, after)];
    let mut seen = HashSet::new();
    while let Some((before, after)) = pending.pop() {
        if after.consumed < before.consumed {
            continue;
        }
        if after.consumed > before.consumed || cursor_after(&before.fc, &after.fc) {
            return true;
        }
        if std::mem::discriminant(&before.fc) != std::mem::discriminant(&after.fc) {
            continue;
        }
        let (mut pairs, completed) = matching_children(&before.fc, &after.fc);
        if completed {
            return true;
        }
        for old in before.oof.iter().filter(|oof| !oof.repeat_fixed) {
            let Some(new) = after.oof.iter().find(|new| new.idx == old.idx) else {
                return true;
            };
            if new.block_start < old.block_start {
                return true;
            }
            if let (Some(a), Some(b)) = (old.token, new.token) {
                pairs.push((a, b));
            }
        }
        for (a, b) in pairs {
            if a != b
                && seen.insert((a, b))
                && let (Some(a), Some(b)) = (store.get(a), store.get(b))
            {
                pending.push((a, b));
            }
        }
    }
    false
}

/// Номер параллельного ребёнка устойчив; позиция в сокращённой очереди — нет.
fn matching_children(before: &FcState, after: &FcState) -> (Vec<(TokenId, TokenId)>, bool) {
    let mut out = Vec::new();
    let mut one = |a: Option<TokenId>, b: Option<TokenId>| {
        if let (Some(a), Some(b)) = (a, b) {
            out.push((a, b));
        }
    };
    let (a, b): (&[ChildToken], &[ChildToken]) = match (before, after) {
        (FcState::Block(a), FcState::Block(b)) => {
            if a.child_idx == b.child_idx {
                one(a.child, b.child);
            }
            (&a.parallel, &b.parallel)
        }
        (FcState::Inline(a), FcState::Inline(b)) => {
            one(a.child, b.child);
            (&[], &[])
        }
        (FcState::Flex(a), FcState::Flex(b)) => (&a.items, &b.items),
        (FcState::Grid(a), FcState::Grid(b)) => (&a.items, &b.items),
        (FcState::Table(a), FcState::Table(b)) => (&a.cells, &b.cells),
        (FcState::Multicol(a), FcState::Multicol(b)) => {
            one(a.flow, b.flow);
            (&[], &[])
        }
        _ => (&[], &[]),
    };
    let mut i = 0;
    let mut j = 0;
    let mut completed = false;
    while i < a.len() && j < b.len() {
        match a[i].idx.cmp(&b[j].idx) {
            std::cmp::Ordering::Less => {
                completed = true;
                i += 1;
            }
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                out.push((a[i].token, b[j].token));
                i += 1;
                j += 1;
            }
        }
    }
    (out, completed || i < a.len())
}

#[cfg(test)]
mod tests;

fn cursor_after(before: &FcState, after: &FcState) -> bool {
    match (before, after) {
        (FcState::Block(a), FcState::Block(b)) => {
            let (start, end) = a.sliced_edges.unwrap_or_default();
            let (next_start, next_end) = b.sliced_edges.unwrap_or_default();
            b.child_idx > a.child_idx || next_start > start || next_end > end
        }
        (FcState::Inline(a), FcState::Inline(b)) => {
            (b.text_pos, b.line_idx) > (a.text_pos, a.line_idx)
        }
        (FcState::Flex(a), FcState::Flex(b)) => b.line > a.line,
        (FcState::Grid(a), FcState::Grid(b)) => b.row_track > a.row_track,
        (FcState::Table(a), FcState::Table(b)) => {
            (b.phase as u8, b.section, b.row) > (a.phase as u8, a.section, a.row)
                || b.consumed_box > a.consumed_box
        }
        (FcState::Multicol(a), FcState::Multicol(b)) => b.row > a.row,
        _ => false,
    }
}
