//! Узел текста дерева коробок: его содержимое и номер исходного текстового
//! узла DOM, из которого он построен.
//!
//! `TextId` is the persistent source NodeKey, not a traversal ordinal. Skipping
//! or moving a sibling cannot change an existing text's identity. Generated
//! and coalesced text has no single source identity.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::{Deref, DerefMut};

/// Persistent identity within the source document, independent of traversal.
pub type TextId = NonZeroU64;

#[derive(Clone, Default)]
pub struct Text {
    text: String,
    source: Option<TextId>,
    /// The source node the text starts with: a glued text keeps it.
    first: Option<TextId>,
}

impl Text {
    /// Текст из исходного узла `source`.
    pub fn from_source(text: String, source: Option<TextId>) -> Self {
        Self {
            text,
            source,
            first: source,
        }
    }

    /// The source text node this text starts with, glued or not: the key of
    /// the flow that publishes it (`BoxTree` flows).
    pub fn first_source(&self) -> Option<TextId> {
        self.first
    }

    /// Номер исходного текстового узла, если текст построен из одного узла.
    pub fn source(&self) -> Option<TextId> {
        self.source
    }

    /// Дописать `more`; склеенный текст больше не принадлежит одному узлу.
    pub fn append(&mut self, more: &str) {
        self.text.push_str(more);
        self.source = None;
    }

    pub fn into_string(self) -> String {
        self.text
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl Deref for Text {
    type Target = String;
    fn deref(&self) -> &String {
        &self.text
    }
}

impl DerefMut for Text {
    fn deref_mut(&mut self) -> &mut String {
        &mut self.text
    }
}

impl From<String> for Text {
    fn from(text: String) -> Self {
        Self {
            text,
            source: None,
            first: None,
        }
    }
}

impl From<&str> for Text {
    fn from(text: &str) -> Self {
        text.to_string().into()
    }
}

impl From<Text> for String {
    fn from(text: Text) -> Self {
        text.text
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl std::borrow::Borrow<str> for Text {
    fn borrow(&self) -> &str {
        &self.text
    }
}

/// Равенство по содержимому: номер источника — не часть текста.
impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for Text {}

impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

impl PartialEq<String> for Text {
    fn eq(&self, other: &String) -> bool {
        &self.text == other
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// Как у `String`; номер исходного узла — после `#`.
impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.text, f)?;
        match self.source {
            Some(id) => write!(f, "#{id}"),
            None => Ok(()),
        }
    }
}

#[cfg(feature = "heap-report")]
kamin_heap::heap_fields!(Text { text });
