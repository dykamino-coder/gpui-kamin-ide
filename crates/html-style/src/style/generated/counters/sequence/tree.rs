//! Path-copying AVL operations; unshared branches mutate in place.
use super::{Link, Node};
use std::rc::Rc;

pub(super) fn len<T: Clone>(node: &Link<T>) -> usize {
    node.as_ref().map_or(0, |n| n.len)
}
fn height<T: Clone>(node: &Link<T>) -> u8 {
    node.as_ref().map_or(0, |n| n.height)
}
fn refresh<T: Clone>(root: &mut Rc<Node<T>>) {
    let n = Rc::make_mut(root);
    n.len = 1 + len(&n.left) + len(&n.right);
    n.height = 1 + height(&n.left).max(height(&n.right));
}
fn left<T: Clone>(mut root: Rc<Node<T>>) -> Rc<Node<T>> {
    let mut r = Rc::make_mut(&mut root)
        .right
        .take()
        .expect("right rotation child");
    Rc::make_mut(&mut root).right = Rc::make_mut(&mut r).left.take();
    refresh(&mut root);
    Rc::make_mut(&mut r).left = Some(root);
    refresh(&mut r);
    r
}
fn right<T: Clone>(mut root: Rc<Node<T>>) -> Rc<Node<T>> {
    let mut l = Rc::make_mut(&mut root)
        .left
        .take()
        .expect("left rotation child");
    Rc::make_mut(&mut root).left = Rc::make_mut(&mut l).right.take();
    refresh(&mut root);
    Rc::make_mut(&mut l).right = Some(root);
    refresh(&mut l);
    l
}
fn balanced<T: Clone>(mut root: Rc<Node<T>>) -> Rc<Node<T>> {
    refresh(&mut root);
    if height(&root.left) > height(&root.right) + 1 {
        let l = root.left.as_ref().expect("heavy left");
        if height(&l.right) > height(&l.left) {
            let n = Rc::make_mut(&mut root);
            n.left = n.left.take().map(left);
        }
        return right(root);
    }
    if height(&root.right) > height(&root.left) + 1 {
        let r = root.right.as_ref().expect("heavy right");
        if height(&r.left) > height(&r.right) {
            let n = Rc::make_mut(&mut root);
            n.right = n.right.take().map(right);
        }
        return left(root);
    }
    root
}
pub(super) fn insert<T: Clone>(root: Link<T>, index: usize, value: T) -> Rc<Node<T>> {
    let Some(mut root) = root else {
        assert_eq!(index, 0);
        return Rc::new(Node {
            value,
            left: None,
            right: None,
            len: 1,
            height: 1,
        });
    };
    let n = Rc::make_mut(&mut root);
    let count = len(&n.left);
    if index <= count {
        n.left = Some(insert(n.left.take(), index, value));
    } else {
        n.right = Some(insert(n.right.take(), index - count - 1, value));
    }
    balanced(root)
}
pub(super) fn remove<T: Clone>(mut root: Rc<Node<T>>, index: usize) -> (Link<T>, T) {
    let n = Rc::make_mut(&mut root);
    let count = len(&n.left);
    let value;
    if index < count {
        let (child, removed) = remove(n.left.take().expect("left"), index);
        n.left = child;
        value = removed;
    } else if index > count {
        let (child, removed) = remove(n.right.take().expect("right"), index - count - 1);
        n.right = child;
        value = removed;
    } else if n.left.is_none() || n.right.is_none() {
        let n = Rc::unwrap_or_clone(root);
        return (n.left.or(n.right), n.value);
    } else {
        let (child, successor) = remove(n.right.take().expect("successor"), 0);
        n.right = child;
        value = std::mem::replace(&mut n.value, successor);
    }
    (Some(balanced(root)), value)
}
pub(super) fn replace<T: Clone>(root: &mut Rc<Node<T>>, index: usize, value: T) {
    let n = Rc::make_mut(root);
    let count = len(&n.left);
    match index.cmp(&count) {
        std::cmp::Ordering::Less => replace(n.left.as_mut().expect("left"), index, value),
        std::cmp::Ordering::Greater => {
            replace(n.right.as_mut().expect("right"), index - count - 1, value)
        }
        std::cmp::Ordering::Equal => n.value = value,
    }
}
