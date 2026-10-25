//! Обход области счётчика с продолжениями на куче и единым путём предков.

use super::{Scan, pseudo_style};
use crate::style::select::{Ancestor, Sibs, census_of};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

enum Work {
    Enter(Handle, Ancestor, Rc<Vec<Ancestor>>, usize, bool),
    Leave(Ancestor, usize),
}

impl Scan<'_> {
    pub(super) fn node(
        &mut self,
        h: &Handle,
        me: &Ancestor,
        path: &[Ancestor],
        sibs: Sibs,
        root: bool,
    ) {
        let mut path = path.to_vec();
        let mut work = vec![Work::Enter(
            h.clone(),
            me.clone(),
            Rc::new(sibs.all.to_vec()),
            sibs.pos,
            root,
        )];
        while let Some(next) = work.pop() {
            if self.done {
                break;
            }
            match next {
                Work::Leave(me, len) => {
                    path.truncate(len);
                    if let Some(style) = pseudo_style(
                        &me,
                        &path,
                        Sibs::EMPTY,
                        "after",
                        self.rules,
                        self.vars,
                        self.vcx,
                    ) {
                        self.step(&style, false);
                    }
                }
                Work::Enter(handle, me, all, pos, root) => {
                    let sibs = Sibs {
                        all: &all,
                        pos,
                        is_elem: true,
                        rc: Some(&all),
                    };
                    let Some(me) = self.enter(&handle, &me, &path, sibs, root) else {
                        continue;
                    };
                    let len = path.len();
                    work.push(Work::Leave(me.clone(), len));
                    path.push(me);
                    let children = handle.children.borrow();
                    let (spots, all) = census_of(&children, self.tree);
                    let all = Rc::new(all);
                    let mut pos = all.len();
                    for (child, spot) in children.iter().zip(&spots).rev() {
                        if spot.index == 0 {
                            continue;
                        }
                        pos -= 1;
                        work.push(Work::Enter(
                            child.clone(),
                            all[pos].clone(),
                            all.clone(),
                            pos,
                            false,
                        ));
                    }
                }
            }
        }
    }
}
