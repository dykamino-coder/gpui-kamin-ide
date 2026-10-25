//! Мемоизация сквозных струн в пределах неизменяемого обхода цепочки полей.

use super::{Strut, adjoin, own_strut};
use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::box_kind::{in_flow, inline_marked_block};
use crate::style::computed::Display;
use std::collections::HashMap;

type Key = (usize, bool, u32, Option<u32>, bool);
#[derive(Default)]
pub(crate) struct ThroughCache(HashMap<Key, Option<Strut>>);

fn key(e: &Element, ignore: bool, vcx: ValueCx<'_>) -> Key {
    (
        std::ptr::from_ref(e) as usize,
        ignore,
        vcx.collapse.font.to_bits(),
        vcx.collapse.width.map(f32::to_bits),
        vcx.collapse.height_def,
    )
}

enum Work<'a> {
    Enter(&'a Element, bool),
    Leave(Key, Strut, Vec<Key>, bool),
}
impl ThroughCache {
    pub(crate) fn get(&mut self, e: &Element, ignore: bool, vcx: ValueCx<'_>) -> Option<Strut> {
        let root = key(e, ignore, vcx);
        let mut pending = vec![Work::Enter(e, ignore)];
        while let Some(work) = pending.pop() {
            match work {
                Work::Enter(e, ignore) => {
                    let id = key(e, ignore, vcx);
                    if self.0.contains_key(&id) {
                        continue;
                    }
                    let Some(own) = own_strut(e, ignore, vcx) else {
                        self.0.insert(id, None);
                        continue;
                    };
                    let mut children = Vec::new();
                    let mut descend = Vec::new();
                    let mut blocked = false;
                    for child in &e.children {
                        let Node::Element(ch) = child else { continue };
                        if in_flow(&ch.style.used()) && ch.style.used().clear.is_some() {
                            blocked = true;
                            break;
                        }
                        if (ch.inline && !inline_marked_block(ch))
                            || matches!(
                                ch.style.used().display,
                                Some(Display::None | Display::Contents)
                            )
                            || !in_flow(&ch.style.used())
                        {
                            continue;
                        }
                        children.push(key(ch, false, vcx));
                        descend.push(Work::Enter(ch, false));
                    }
                    if blocked {
                        self.0.insert(id, None);
                        continue;
                    }
                    pending.push(Work::Leave(
                        id,
                        own,
                        children,
                        e.style.used().margin_trim & 3 == 0,
                    ));
                    pending.extend(descend);
                }
                Work::Leave(id, mut strut, children, contributes) => {
                    let mut valid = true;
                    for child in children {
                        if let Some(value) = self.0[&child] {
                            if contributes {
                                strut = adjoin(strut, value);
                            }
                        } else {
                            valid = false;
                            break;
                        }
                    }
                    self.0.insert(id, valid.then_some(strut));
                }
            }
        }
        self.0[&root]
    }
}
