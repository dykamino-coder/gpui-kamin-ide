//! Owned events retain every parser reservation until their last owner drops.

use super::{
    control::Control,
    storage::{Storage, Text},
};
use crate::{
    attribute::OwnedAttribute,
    common::XmlVersion,
    name::{Name as BorrowedName, OwnedName},
    reader::{Error, XmlEvent},
};
use std::fmt;

/// An admitted qualified XML name.
#[derive(Debug, PartialEq, Eq)]
pub struct Name {
    /// Local spelling.
    pub local_name: Text,
    /// Resolved namespace URI.
    pub namespace: Option<Text>,
    /// Prefix spelling.
    pub prefix: Option<Text>,
}

impl Name {
    pub(crate) fn parse(value: &str, control: &Control) -> Result<Option<Self>, Error> {
        control.work(value.len())?;
        let mut parts = value.split(':');
        let (local, prefix) = match (parts.next(), parts.next(), parts.next()) {
            (Some(prefix), Some(local), None) if !prefix.is_empty() && !local.is_empty() => {
                (local, Some(prefix))
            }
            (Some(local), None, None) if !local.is_empty() => (local, None),
            _ => return Ok(None),
        };
        control.name(local)?;
        if let Some(prefix) = prefix {
            control.name(prefix)?;
        }
        Ok(Some(Self {
            local_name: Text::from_str(local, control)?,
            namespace: None,
            prefix: prefix.map(|p| Text::from_str(p, control)).transpose()?,
        }))
    }
    pub(crate) fn matches(&self, other: &Self, control: &Control) -> Result<bool, Error> {
        Ok(self.compare(other, control)?.is_eq())
    }
    pub(crate) fn compare(
        &self,
        other: &Self,
        control: &Control,
    ) -> Result<std::cmp::Ordering, Error> {
        for (left, right) in [
            (
                Some(self.local_name.as_str()),
                Some(other.local_name.as_str()),
            ),
            (self.namespace.as_deref(), other.namespace.as_deref()),
            (self.prefix.as_deref(), other.prefix.as_deref()),
        ] {
            control.work(
                left.map_or(0, str::len)
                    .min(right.map_or(0, str::len))
                    .saturating_add(1),
            )?;
            let order = left.cmp(&right);
            if !order.is_eq() {
                return Ok(order);
            }
        }
        Ok(std::cmp::Ordering::Equal)
    }
    pub(crate) fn compare_expanded(
        &self,
        other: &Self,
        control: &Control,
    ) -> Result<std::cmp::Ordering, Error> {
        for (left, right) in [
            (
                Some(self.local_name.as_str()),
                Some(other.local_name.as_str()),
            ),
            (self.namespace.as_deref(), other.namespace.as_deref()),
        ] {
            control.work(
                left.map_or(0, str::len)
                    .min(right.map_or(0, str::len))
                    .saturating_add(1),
            )?;
            let order = left.cmp(&right);
            if !order.is_eq() {
                return Ok(order);
            }
        }
        Ok(std::cmp::Ordering::Equal)
    }
    pub(crate) fn borrow(&self) -> BorrowedName<'_> {
        BorrowedName {
            local_name: &self.local_name,
            namespace: self.namespace.as_deref(),
            prefix: self.prefix.as_deref(),
        }
    }
    pub(crate) fn prefix_ref(&self) -> Option<&str> {
        self.prefix.as_deref()
    }
    pub(crate) fn try_clone(&self, control: &Control) -> Result<Self, Error> {
        Ok(Self {
            local_name: self.local_name.try_clone(control)?,
            namespace: self
                .namespace
                .as_ref()
                .map(|s| s.try_clone(control))
                .transpose()?,
            prefix: self
                .prefix
                .as_ref()
                .map(|s| s.try_clone(control))
                .transpose()?,
        })
    }
    fn into_legacy(self) -> OwnedName {
        OwnedName {
            local_name: self.local_name.into_legacy(),
            namespace: self.namespace.map(Text::into_legacy),
            prefix: self.prefix.map(Text::into_legacy),
        }
    }
}
impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.borrow().fmt(f)
    }
}

/// An admitted attribute; its name and value own their respective reservations.
#[derive(Debug)]
pub struct Attribute {
    /// Qualified name.
    pub name: Name,
    /// Normalized value.
    pub value: Text,
}

/// Namespace snapshot in ascending prefix order.
pub type Namespace = Storage<(Text, Text)>;

/// The canonical core's event representation, without legacy allocation conversion.
#[derive(Debug)]
pub enum Event {
    /// XML declaration (including an implied declaration).
    StartDocument {
        /// XML version.
        version: XmlVersion,
        /// Encoding label.
        encoding: Text,
        /// Standalone declaration.
        standalone: Option<bool>,
    },
    /// Terminal document event.
    EndDocument,
    /// Processing instruction.
    ProcessingInstruction {
        /// Target.
        name: Text,
        /// Content.
        data: Option<Text>,
    },
    /// Opening tag and the namespace snapshot at this element.
    StartElement {
        /// Resolved name.
        name: Name,
        /// Attributes in source order.
        attributes: Storage<Attribute>,
        /// Active namespaces.
        namespace: Namespace,
    },
    /// Closing tag.
    EndElement {
        /// Resolved name.
        name: Name,
    },
    /// CDATA content.
    CData(Text),
    /// Comment content.
    Comment(Text),
    /// Character content.
    Characters(Text),
    /// Whitespace content.
    Whitespace(Text),
}

impl Event {
    /// Conversion is exclusively for the unbounded legacy EventReader boundary.
    pub(crate) fn into_legacy(self) -> XmlEvent {
        match self {
            Self::StartDocument {
                version,
                encoding,
                standalone,
            } => XmlEvent::StartDocument {
                version,
                encoding: encoding.into_legacy(),
                standalone,
            },
            Self::EndDocument => XmlEvent::EndDocument,
            Self::ProcessingInstruction { name, data } => XmlEvent::ProcessingInstruction {
                name: name.into_legacy(),
                data: data.map(Text::into_legacy),
            },
            Self::StartElement {
                name,
                attributes,
                namespace,
            } => XmlEvent::StartElement {
                name: name.into_legacy(),
                attributes: attributes
                    .into_legacy()
                    .into_iter()
                    .map(|a| OwnedAttribute {
                        name: a.name.into_legacy(),
                        value: a.value.into_legacy(),
                    })
                    .collect(),
                namespace: crate::namespace::Namespace(
                    namespace
                        .into_legacy()
                        .into_iter()
                        .map(|(p, u)| (p.into_legacy(), u.into_legacy()))
                        .collect(),
                ),
            },
            Self::EndElement { name } => XmlEvent::EndElement {
                name: name.into_legacy(),
            },
            Self::CData(text) => XmlEvent::CData(text.into_legacy()),
            Self::Comment(text) => XmlEvent::Comment(text.into_legacy()),
            Self::Characters(text) => XmlEvent::Characters(text.into_legacy()),
            Self::Whitespace(text) => XmlEvent::Whitespace(text.into_legacy()),
        }
    }
}
