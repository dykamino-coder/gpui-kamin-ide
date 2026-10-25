//! Итеративная запись дерева parallel tokens в порядке завершения детей.

use super::children::Child;
use super::*;

struct Frame<'a> {
    b: &'a BlockBox,
    children: Vec<Child<'a>>,
    next: usize,
    visited: usize,
    state: BlockState,
    top: f32,
    bottom: f32,
    done: bool,
}

impl<'a> Frame<'a> {
    fn new(b: &'a BlockBox, laid: &Laid, from: f32) -> Result<Option<Self>, Error> {
        let Some((top, bottom)) = laid.at(b.id) else {
            return Ok(None);
        };
        if !from.is_finite()
            || !top.is_finite()
            || !bottom.is_finite()
            || bottom < top
            || ![b.head, b.tail].iter().all(|v| v.is_finite() && *v >= 0.0)
        {
            return Err(Error::InvalidGeometry);
        }
        let children = children(b);
        let done = bottom <= from + COORD_EPS;
        let next = if done {
            children.len()
        } else {
            children
                .iter()
                .rposition(|child| laid.at(child.id).is_some_and(|(top, _)| top < from))
                .map_or(0, |idx| idx + 1)
        };
        Ok(Some(Self {
            b,
            children,
            next,
            visited: 0,
            state: BlockState::default(),
            top,
            bottom,
            done,
        }))
    }

    fn append(&mut self, token: TokenId) -> Result<(), Error> {
        self.state.parallel.push(ChildToken {
            idx: u32::try_from(self.visited - 1).map_err(|_| Error::ResourceExhausted)?,
            token,
        });
        Ok(())
    }
}

impl Parallel {
    pub(super) fn record(
        &mut self,
        b: &BlockBox,
        laid: &Laid,
        from: f32,
        previous: Option<(&Self, bool)>,
    ) -> Result<Option<TokenId>, Error> {
        let Some(root) = Frame::new(b, laid, from)? else {
            return Ok(None);
        };
        let mut stack = vec![root];
        while let Some(mut frame) = stack.pop() {
            if frame.visited < frame.next {
                let child = &frame.children[frame.visited];
                let (id, monolith, inner) = (child.id, child.monolith, child.inner);
                frame.visited += 1;
                if let Some((top, bottom)) = laid.at(id) {
                    if !monolith && let Some(inner) = inner {
                        let child = Frame::new(inner, laid, from)?;
                        stack.push(frame);
                        if let Some(child) = child {
                            stack.push(child);
                        }
                        continue;
                    } else if bottom > from + COORD_EPS
                        && let Some(token) = self.leaf(id, top, bottom, from, previous)?
                    {
                        frame.append(token)?;
                    }
                }
                stack.push(frame);
                continue;
            }
            let result = self.finish(frame, from, previous)?;
            if let Some(parent) = stack.last_mut() {
                if let Some(token) = result {
                    parent.append(token)?;
                }
            } else {
                return Ok(result);
            }
        }
        unreachable!("root frame returns its token")
    }

    fn finish(
        &mut self,
        frame: Frame<'_>,
        from: f32,
        previous: Option<(&Self, bool)>,
    ) -> Result<Option<TokenId>, Error> {
        let Frame {
            b,
            children,
            next,
            mut state,
            top,
            bottom,
            done,
            ..
        } = frame;
        let children_len = children.len();
        if done && (b.clips_block || state.parallel.is_empty()) {
            return Ok(None);
        }
        state.child_idx = u32::try_from(next).map_err(|_| Error::ResourceExhausted)?;
        // Последний начатый normal ребёнок — основной; более ранние
        // незавершённые дети продолжаются параллельно ему.
        if !done
            && let Some(last) = state.parallel.last()
            && usize::try_from(last.idx).ok() == next.checked_sub(1)
            && !self
                .store
                .get(last.token)
                .ok_or(Error::InvalidToken)?
                .at_block_end
        {
            let last = state.parallel.pop().unwrap();
            state.child_idx = last.idx;
            state.child = Some(last.token);
        }
        let seen = next == children_len;
        let mut token = Token::new(FcState::Block(state));
        progress::record(&mut token, bottom - top, (b.head, b.tail), from - top);
        token.at_block_end = done;
        token.flags.seen_all_children = seen;
        token.flags.break_before = from <= top;
        if token.flags.break_before {
            token.seq = 0;
        }
        self.save(b.id, token, previous).map(Some)
    }

    fn leaf(
        &mut self,
        id: u64,
        top: f32,
        bottom: f32,
        from: f32,
        previous: Option<(&Self, bool)>,
    ) -> Result<Option<TokenId>, Error> {
        if !top.is_finite() || !bottom.is_finite() || bottom < top {
            return Err(Error::InvalidGeometry);
        }
        let mut token = Token::new(FcState::Block(BlockState::default()));
        progress::record(&mut token, bottom - top, (0.0, 0.0), from - top);
        token.flags.break_before = from <= top;
        if token.flags.break_before {
            token.seq = 0;
        }
        self.save(id, token, previous).map(Some)
    }

    fn save(
        &mut self,
        id: u64,
        mut token: Token,
        previous: Option<(&Self, bool)>,
    ) -> Result<TokenId, Error> {
        if !token.flags.break_before
            && let Some((old, advance)) = previous
            && let Some(&key) = old.ids.get(&id)
            && let Some(t) = old.store.get(key)
            && !t.flags.break_before
        {
            token.seq = t
                .seq
                .checked_add(u32::from(advance))
                .ok_or(Error::ResourceExhausted)?;
        }
        let key = self.store.push(token)?;
        self.ids.insert(id, key);
        Ok(key)
    }
}
