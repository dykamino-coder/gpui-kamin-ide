//! Предпроход областей теней с продолжениями и восстановлением пути.

use super::{Scope, Shadow, SlotInfo, assign_slots, declarative_shadow, prepare, slotted};
use crate::dom::tree_index::{NodeKey, TreeIndex};
use crate::style::select::has::{HasArg, shadow_marks};
use crate::style::select::{Ancestor, census_of};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

/// Предпроход по теням в порядке дерева. На хосте: область стилей тени,
/// отметки `:has` по тени, распределение слотов; дальше — в тень с цепочкой
/// из одного безликого хоста (css-shadow-1 §3.1: «the selector match list is
/// initially the shadow host, followed by all children of the shadow root»)
/// и в светлых детей — в текущей области.
pub(in crate::dom) fn scan_shadows(
    children: &[Handle],
    path: &mut Vec<Ancestor>,
    scope: &Rc<Scope>,
    prepared: &prepare::Prepared,
    args: &[HasArg],
    drafts: &mut HashMap<NodeKey, SlotInfo>,
    tree: &mut TreeIndex,
) {
    enum Work {
        Level(Vec<Handle>, Rc<Scope>),
        Visit(Handle, Rc<Vec<Ancestor>>, usize, Rc<Scope>),
        Children(Vec<Handle>, Ancestor, Rc<Scope>),
        Shadow(Vec<Handle>, Ancestor, Rc<Scope>),
        Truncate(usize),
        Restore(Vec<Ancestor>),
    }
    let mut work = vec![Work::Level(children.to_vec(), scope.clone())];
    while let Some(next) = work.pop() {
        let (child, all, pos, scope) = match next {
            Work::Truncate(len) => {
                path.truncate(len);
                continue;
            }
            Work::Restore(old) => {
                *path = old;
                continue;
            }
            Work::Children(children, me, scope) => {
                work.push(Work::Truncate(path.len()));
                path.push(me);
                work.push(Work::Level(children, scope));
                continue;
            }
            Work::Shadow(children, marker, scope) => {
                work.push(Work::Restore(std::mem::replace(path, vec![marker])));
                work.push(Work::Level(children, scope));
                continue;
            }
            Work::Level(children, scope) => {
                let (_, all) = census_of(&children, tree);
                let all = Rc::new(all);
                let mut pos = all.len();
                for child in children.into_iter().rev() {
                    if !matches!(&child.data, NodeData::Element { .. }) {
                        continue;
                    }
                    pos -= 1;
                    work.push(Work::Visit(child, all.clone(), pos, scope.clone()));
                }
                continue;
            }
            Work::Visit(child, all, pos, scope) => (child, all, pos, scope),
        };
        let mut me = all[pos].clone();
        me.peers = Some((all.clone(), pos));
        if let Some(slot) = drafts.get_mut(&NodeKey::of(&child)) {
            slot.origin = Some(slotted::SlotContext {
                scope: scope.clone(),
                node: me.clone(),
                path: path.clone(),
                peers: all.clone(),
                pos,
            });
        }
        if let Some((template, root)) = declarative_shadow(&child) {
            let inner = prepared[&NodeKey::of(&child)].1.clone();
            let host_path: Rc<Vec<Ancestor>> = Rc::new(
                path.iter()
                    .cloned()
                    .chain(std::iter::once(me.clone()))
                    .collect(),
            );
            let light: Vec<Handle> = child
                .children
                .borrow()
                .iter()
                .filter(|c| !Rc::ptr_eq(c, &template))
                .cloned()
                .collect();
            assign_slots(&root, &light, &scope, &host_path, drafts, tree);
            let marker = Ancestor {
                featureless: Some(Rc::new(path.clone())),
                shadow_has_marks: shadow_marks(&root, args, &me, tree),
                ..me.clone()
            };
            tree.shadows.insert(
                NodeKey::of(&child),
                Rc::new(Shadow {
                    root: root.clone(),
                    scope: inner.clone(),
                    marker: marker.clone(),
                }),
            );
            let shadow_kids: Vec<Handle> = root.children.borrow().clone();
            work.push(Work::Children(light, me, scope));
            work.push(Work::Shadow(shadow_kids, marker, inner));
        } else {
            let kids: Vec<Handle> = child.children.borrow().clone();
            work.push(Work::Children(kids, me, scope));
        }
    }
}
