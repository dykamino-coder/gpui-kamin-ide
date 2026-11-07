//! A node field shared between the copies box construction makes of an element.
//!
//! Box construction (anonymous boxes, block-in-inline splits, margin collapsing, layers)
//! works on lists of nodes and copies elements it rearranges. CSS Display 3 §2 generates
//! boxes *from* the styled element; Blink's LayoutObject and Servo's box tree reference the
//! node instead of copying it. The tag and attributes of an element never change during
//! construction, so a copy shares them (a reference count) and a rare write copies them
//! (`DerefMut`, copy on write).
//!
//! Construction must not cost more than the plain values did: element names come from a small
//! vocabulary and are interned per thread (Blink and Gecko intern them as atoms), and every
//! empty attribute list is one shared value, so neither allocates per element.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

#[derive(PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Shared<T>(Rc<T>);

/// Attribute list of an element: name, value.
pub type Attrs = Vec<(String, String)>;

/// Interned names are short; a document inventing ever new long names keeps its own copies.
const MAX_INTERNED_LEN: usize = 32;
const MAX_INTERNED: usize = 4096;

thread_local! {
    static NAMES: RefCell<HashMap<String, Rc<String>>> = RefCell::new(HashMap::new());
    static NO_ATTRS: Rc<Attrs> = Rc::new(Vec::new());
}

fn intern<S: AsRef<str> + Into<String>>(name: S) -> Shared<String> {
    if name.as_ref().len() > MAX_INTERNED_LEN {
        return Shared(Rc::new(name.into()));
    }
    NAMES.with(|names| {
        let mut names = names.borrow_mut();
        if let Some(rc) = names.get(name.as_ref()) {
            return Shared(rc.clone());
        }
        let rc = Rc::new(name.into());
        if names.len() < MAX_INTERNED {
            names.insert(String::clone(&rc), rc.clone());
        }
        Shared(rc)
    })
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Shared<T> {
    pub fn new(value: T) -> Self {
        Self(Rc::new(value))
    }
}

impl<T: Clone> Shared<T> {
    /// The value, without a copy when this is its only holder.
    pub fn into_inner(self) -> T {
        Rc::unwrap_or_clone(self.0)
    }
}

impl<T> Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone> DerefMut for Shared<T> {
    fn deref_mut(&mut self) -> &mut T {
        Rc::make_mut(&mut self.0)
    }
}

impl Default for Shared<String> {
    fn default() -> Self {
        intern("")
    }
}

impl Default for Shared<Attrs> {
    fn default() -> Self {
        Self(NO_ATTRS.with(Rc::clone))
    }
}

impl From<String> for Shared<String> {
    fn from(value: String) -> Self {
        intern(value)
    }
}

impl From<&str> for Shared<String> {
    fn from(value: &str) -> Self {
        intern(value)
    }
}

impl From<Attrs> for Shared<Attrs> {
    fn from(value: Attrs) -> Self {
        if value.is_empty() {
            Self::default()
        } else {
            Self::new(value)
        }
    }
}

impl From<Shared<String>> for String {
    fn from(value: Shared<String>) -> Self {
        value.into_inner()
    }
}

impl<T: fmt::Debug> fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl<T: fmt::Display> fmt::Display for Shared<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.0, f)
    }
}

impl PartialEq<str> for Shared<String> {
    fn eq(&self, other: &str) -> bool {
        self.0.as_str() == other
    }
}

impl PartialEq<&str> for Shared<String> {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_str() == *other
    }
}

impl PartialEq<String> for Shared<String> {
    fn eq(&self, other: &String) -> bool {
        *self.0 == *other
    }
}

impl PartialEq<Shared<String>> for str {
    fn eq(&self, other: &Shared<String>) -> bool {
        self == other.0.as_str()
    }
}

impl PartialEq<Shared<String>> for &str {
    fn eq(&self, other: &Shared<String>) -> bool {
        *self == other.0.as_str()
    }
}

impl PartialEq<Shared<String>> for String {
    fn eq(&self, other: &Shared<String>) -> bool {
        *self == *other.0
    }
}

impl AsRef<str> for Shared<String> {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for Shared<String> {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl<'a, T> IntoIterator for &'a Shared<Vec<T>> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<(String, String)> for Shared<Attrs> {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        iter.into_iter().collect::<Attrs>().into()
    }
}

#[cfg(test)]
mod tests {
    use super::Shared;

    #[test]
    fn copies_share_until_one_is_written() {
        let a: Shared<Vec<(String, String)>> = vec![("class".into(), "row".into())].into();
        let mut b = a.clone();
        assert!(std::ptr::eq(&*a, &*b), "a copy shares the attributes");
        b.push(("id".into(), "x".into()));
        assert_eq!(a.len(), 1, "a write does not reach the other copy");
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn names_and_empty_attribute_lists_are_one_value_per_thread() {
        let a: Shared<String> = "div".into();
        let b: Shared<String> = String::from("div").into();
        assert!(std::ptr::eq(&*a, &*b), "an element name is interned");
        let mut c = b.clone();
        c.push_str("x");
        assert_eq!((a.as_str(), c.as_str()), ("div", "divx"), "a write copies");
        let long: Shared<String> = "x".repeat(super::MAX_INTERNED_LEN + 1).into();
        let again: Shared<String> = "x".repeat(super::MAX_INTERNED_LEN + 1).into();
        assert!(!std::ptr::eq(&*long, &*again), "long names are not kept");
        let empty: Shared<super::Attrs> = Vec::new().into();
        assert!(std::ptr::eq(&*empty, &*Shared::<super::Attrs>::default()));
    }

    #[test]
    fn a_shared_tag_compares_like_a_string() {
        let tag: Shared<String> = "div".into();
        assert!(tag == "div");
        assert!("div" == tag);
        assert_eq!(tag.as_str(), "div");
        assert_eq!(String::from(tag.clone()), "div");
    }
}
