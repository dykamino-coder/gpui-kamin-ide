//! Узел текста дерева коробок: его содержимое и номер исходного текстового
//! узла DOM, из которого он построен.
//!
//! Номер (`TextId`) даёт обход при сборке, как `node_id` элементу: порядковый
//! номер исходного текста в порядке обхода, начиная с единицы. Одинаковая по
//! форме разметка получает одинаковые номера. Куски и копии одного исходного
//! текста несут его номер; текст, склеенный из нескольких исходных, и текст,
//! созданный сборкой (`content`, маркеры, пробелы-заполнители), — без номера.

use std::fmt;
use std::num::NonZeroU32;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

/// Номер исходного текстового узла документа.
pub type TextId = NonZeroU32;

/// The content is shared between the copies box construction makes of a node (fixup passes,
/// anonymous boxes, pieces): a copy costs a reference count, a write copies (CSS Display 3 §2:
/// boxes are generated from the styled node, not from copies of it).
#[derive(Clone, Default)]
pub struct Text {
    text: Rc<String>,
    source: Option<TextId>,
}

impl Text {
    /// Текст из исходного узла `source`.
    pub fn from_source(text: String, source: Option<TextId>) -> Self {
        Self {
            text: Rc::new(text),
            source,
        }
    }

    /// Номер исходного текстового узла, если текст построен из одного узла.
    pub fn source(&self) -> Option<TextId> {
        self.source
    }

    /// Дописать `more`; склеенный текст больше не принадлежит одному узлу.
    pub fn append(&mut self, more: &str) {
        Rc::make_mut(&mut self.text).push_str(more);
        self.source = None;
    }

    pub fn into_string(self) -> String {
        Rc::unwrap_or_clone(self.text)
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
        Rc::make_mut(&mut self.text)
    }
}

impl From<String> for Text {
    fn from(text: String) -> Self {
        Self {
            text: Rc::new(text),
            source: None,
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
        text.into_string()
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
        *self.text == other
    }
}

impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        *self.text == *other
    }
}

impl PartialEq<String> for Text {
    fn eq(&self, other: &String) -> bool {
        &*self.text == other
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
