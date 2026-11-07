//! A node field shared between the copies box construction makes of an element.
//!
//! Box construction (anonymous boxes, block-in-inline splits, margin collapsing, layers)
//! works on lists of nodes and copies elements it rearranges. CSS Display 3 §2 generates
//! boxes *from* the styled element; Blink's LayoutObject and Servo's box tree reference the
//! node instead of copying it. The tag and attributes of an element never change during
//! construction, so a copy shares them (a reference count). A rare writer replaces the tag or
//! edits a copy of the attributes ([`SharedAttrs::edit`]).
//!
//! Construction must not cost more than the plain values did: element names come from a small
//! vocabulary and are interned per thread (Blink and Gecko intern them as atoms), and every
//! empty attribute list is one shared value, so neither allocates per element. A non-empty
//! attribute list is one allocation holding the count and the pairs (`Rc<[Attr]>`), no larger
//! than the vector it replaces.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

#[derive(PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Shared<T: ?Sized>(Rc<T>);

/// One attribute of an element: name, value.
pub type Attr = (String, String);

/// Attribute list of an element being assembled or edited.
pub type Attrs = Vec<Attr>;

/// Attribute list held by an element and shared by its copies.
pub type SharedAttrs = Shared<[Attr]>;

/// Interned names are short; a document inventing ever new long names keeps its own copies.
const MAX_INTERNED_LEN: usize = 32;
const MAX_INTERNED: usize = 4096;

thread_local! {
    static NAMES: RefCell<HashMap<String, Rc<String>>> = RefCell::new(HashMap::new());
    static NO_ATTRS: Rc<[Attr]> = Rc::from(Vec::new());
}

fn intern<S: AsRef<str> + Into<String>>(name: S) -> Shared<String> {
    if name.as_ref().len() > MAX_INTERNED_LEN {
        return Shared(Rc::new(name.into()));
    }
    NAMES.with_borrow_mut(|names| {
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

impl<T: ?Sized> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Shared<T> {
    pub fn new(value: T) -> Self {
        Self(Rc::new(value))
    }
}

impl<T: ?Sized> Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl Default for Shared<String> {
    fn default() -> Self {
        intern("")
    }
}

impl SharedAttrs {
    /// A copy of the list for changing, adding or removing attributes; it replaces this
    /// element's list when the returned guard drops, and other holders keep theirs.
    pub fn edit(&mut self) -> AttrsEdit<'_> {
        let list = self.0.to_vec();
        AttrsEdit { target: self, list }
    }
}

/// Attribute list being edited through [`SharedAttrs::edit`].
pub struct AttrsEdit<'a> {
    target: &'a mut SharedAttrs,
    list: Attrs,
}

impl Deref for AttrsEdit<'_> {
    type Target = Attrs;
    fn deref(&self) -> &Attrs {
        &self.list
    }
}

impl DerefMut for AttrsEdit<'_> {
    fn deref_mut(&mut self) -> &mut Attrs {
        &mut self.list
    }
}

impl Drop for AttrsEdit<'_> {
    fn drop(&mut self) {
        *self.target = std::mem::take(&mut self.list).into();
    }
}

impl Default for SharedAttrs {
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

impl From<Attrs> for SharedAttrs {
    fn from(value: Attrs) -> Self {
        if value.is_empty() {
            Self::default()
        } else {
            Self(Rc::from(value))
        }
    }
}

impl From<Shared<String>> for String {
    fn from(value: Shared<String>) -> Self {
        String::clone(&value.0)
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for Shared<T> {
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

impl<'a, T> IntoIterator for &'a Shared<[T]> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<Attr> for SharedAttrs {
    fn from_iter<I: IntoIterator<Item = Attr>>(iter: I) -> Self {
        iter.into_iter().collect::<Attrs>().into()
    }
}

#[cfg(test)]
mod tests;
