//! Element name and attribute storage of styled nodes.
//!
//! Box construction (anonymous boxes, block-in-inline splits, margin collapsing, layers)
//! works on lists of nodes and copies elements it rearranges. CSS Display 3 §2 generates
//! boxes *from* the styled element; Blink's LayoutObject and Servo's box tree reference the
//! node instead of copying it. The tag of an element never changes during construction, so a
//! copy shares it. A rare writer replaces the tag or edits the attributes ([`ElementAttrs::edit`]).
//!
//! Construction must not cost more than the plain values did. An element name is the parser's
//! own atom ([`Tag`] over `LocalName`, as Servo keeps names; Blink and Gecko intern them too):
//! the names of the HTML, SVG and MathML vocabularies are static, short names are inline, so a
//! name costs no allocation per element.
//!
//! Attributes: on the measured corpus almost every element has none or one (selectors-3200:
//! 80 001 of 80 006 elements have one). Blink gives an element without attributes no
//! ElementData at all (third_party/blink/renderer/core/dom/element_data.h:88, `Attributes()`
//! of the shareable and unique variants at :254/:258). Here an empty list is no allocation and
//! a non-empty one is one exact-size block of the pairs with no reference-count header
//! (`Box<[Attr]>`, 16 bytes in the element instead of a vector's 24). A shared `Rc<[Attr]>`
//! made copies free but added a 16-byte header to every element's list, which raised the
//! style-phase heap peak; a copy now costs what a vector copy did.

use html5ever::LocalName;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

/// One attribute of an element: name, value.
pub type Attr = (String, String);

/// Attribute list of an element being assembled or edited.
pub type Attrs = Vec<Attr>;

/// Element name, shared by every copy of the element.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Tag(LocalName);

impl Tag {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for Tag {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

/// Hashes and orders like the name itself, so a `str` finds a `Tag` key.
impl Hash for Tag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl PartialOrd for Tag {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Tag {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl From<&str> for Tag {
    fn from(value: &str) -> Self {
        Self(LocalName::from(value))
    }
}

impl From<String> for Tag {
    fn from(value: String) -> Self {
        Self(LocalName::from(value))
    }
}

impl From<LocalName> for Tag {
    fn from(value: LocalName) -> Self {
        Self(value)
    }
}

impl From<Tag> for String {
    fn from(value: Tag) -> Self {
        value.as_str().to_string()
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<str> for Tag {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Tag {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for Tag {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<Tag> for str {
    fn eq(&self, other: &Tag) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<Tag> for &str {
    fn eq(&self, other: &Tag) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<Tag> for String {
    fn eq(&self, other: &Tag) -> bool {
        self.as_str() == other.as_str()
    }
}

impl AsRef<str> for Tag {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Tag {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

/// Attribute list of an element: empty without an allocation, otherwise one exact-size block.
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementAttrs(Option<Box<[Attr]>>);

impl ElementAttrs {
    /// The list as a vector for changing, adding or removing attributes (no copy: the block
    /// becomes the vector); it is stored back when the returned guard drops.
    pub fn edit(&mut self) -> AttrsEdit<'_> {
        let list = self.0.take().map(Vec::from).unwrap_or_default();
        AttrsEdit { target: self, list }
    }
}

impl Deref for ElementAttrs {
    type Target = [Attr];
    fn deref(&self) -> &[Attr] {
        self.0.as_deref().unwrap_or(&[])
    }
}

/// Attribute list being edited through [`ElementAttrs::edit`].
pub struct AttrsEdit<'a> {
    target: &'a mut ElementAttrs,
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

impl From<Attrs> for ElementAttrs {
    fn from(value: Attrs) -> Self {
        Self((!value.is_empty()).then(|| value.into_boxed_slice()))
    }
}

impl fmt::Debug for ElementAttrs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a> IntoIterator for &'a ElementAttrs {
    type Item = &'a Attr;
    type IntoIter = std::slice::Iter<'a, Attr>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl FromIterator<Attr> for ElementAttrs {
    fn from_iter<I: IntoIterator<Item = Attr>>(iter: I) -> Self {
        iter.into_iter().collect::<Attrs>().into()
    }
}

#[cfg(test)]
mod tests;
