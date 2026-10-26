//! Итеративное чтение parallel tokens после повторного измерения.

use super::*;

struct Frame<'a> {
    work: Vec<(TokenId, &'a BlockBox, bool)>,
    best: Option<f32>,
    fallback: Option<f32>,
}

impl Parallel {
    pub(super) fn position(
        &self,
        key: TokenId,
        b: &BlockBox,
        new: &Laid,
        parallel: bool,
    ) -> Option<f32> {
        let mut stack = vec![self.position_frame(key, b, new, parallel)?];
        loop {
            let frame = stack.last_mut()?;
            if let Some((key, b, parallel)) = frame.work.pop() {
                if let Some(child) = self.position_frame(key, b, new, parallel) {
                    stack.push(child);
                }
                continue;
            }
            let result = frame.best.or(frame.fallback);
            stack.pop();
            if let Some(parent) = stack.last_mut() {
                parent.best = parent.best.into_iter().chain(result).min_by(f32::total_cmp);
            } else {
                return result;
            }
        }
    }

    fn position_frame<'a>(
        &self,
        key: TokenId,
        b: &'a BlockBox,
        new: &Laid,
        parallel: bool,
    ) -> Option<Frame<'a>> {
        let token = self.store.get(key)?;
        let FcState::Block(state) = &token.fc else {
            return None;
        };
        let children = children(b);
        let cursor = ChildCursor::new(
            children.len().try_into().expect("recorded child count"),
            state,
            token.flags.seen_all_children,
        )
        .expect("recorder preserves the child cursor invariant");
        let mut frame = Frame {
            work: Vec::new(),
            best: None,
            fallback: None,
        };
        for child in cursor {
            let Some(k) = children.get(child.index as usize) else {
                continue;
            };
            let Some(key) = child.token else { continue };
            let child_parallel = parallel || token.at_block_end;
            if !k.monolith
                && let Some(inner) = k.inner
            {
                frame.work.push((key, inner, child_parallel));
            } else if child_parallel
                && let Some(t) = self.store.get(key)
                && let Some((top, bottom)) = new.at(k.id)
            {
                let offset = top + progress::offset(t, bottom - top, (0.0, 0.0));
                frame.best = frame
                    .best
                    .into_iter()
                    .chain(Some(offset))
                    .min_by(f32::total_cmp);
            }
        }
        if parallel
            && !token.at_block_end
            && let Some((top, bottom)) = new.at(b.id)
        {
            frame.fallback = Some(top + progress::offset(token, bottom - top, (b.head, b.tail)));
        }
        Some(frame)
    }
}
