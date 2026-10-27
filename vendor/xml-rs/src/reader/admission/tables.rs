//! Fallible namespace, entity and attribute tables for the canonical core.

use super::{control::Control, Attribute, Name, Namespace, Storage, Text};
use crate::{namespace::*, reader::Error};
use std::cmp::Ordering;

fn locate<T>(
    values: &[T],
    key: &str,
    spelling: impl Fn(&T) -> &str,
    control: &Control,
) -> Result<Result<usize, usize>, Error> {
    let (mut low, mut high) = (0, values.len());
    while low < high {
        let middle = low + (high - low) / 2;
        let value = spelling(&values[middle]);
        control.work(value.len().min(key.len()).saturating_add(1))?;
        match value.cmp(key) {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Ok(Ok(middle)),
        }
    }
    Ok(Err(low))
}

pub(crate) struct NamespaceStack {
    bindings: Namespace,
    scopes: Storage<usize>,
}

impl NamespaceStack {
    pub(crate) fn new(control: &Control) -> Result<Self, Error> {
        let mut stack = Self {
            bindings: Storage::new(),
            scopes: Storage::new(),
        };
        stack.push_empty(control)?;
        for (prefix, uri) in [
            (NS_NO_PREFIX, NS_EMPTY_URI),
            (NS_XMLNS_PREFIX, NS_XMLNS_URI),
            (NS_XML_PREFIX, NS_XML_URI),
        ] {
            stack.put(
                Text::from_str(prefix, control)?,
                Text::from_str(uri, control)?,
                control,
            )?;
        }
        Ok(stack)
    }
    pub(crate) fn push_empty(&mut self, control: &Control) -> Result<(), Error> {
        self.scopes.push(self.bindings.len(), control)
    }
    pub(crate) fn pop(&mut self) {
        if let Some(start) = self.scopes.pop() {
            self.bindings.truncate(start);
        }
    }
    pub(crate) fn get(&self, prefix: &str, control: &Control) -> Result<Option<&str>, Error> {
        for (key, uri) in self.bindings.iter().rev() {
            control.work(key.len().min(prefix.len()).saturating_add(1))?;
            if key.as_str() == prefix {
                return Ok(Some(uri));
            }
        }
        Ok(None)
    }
    pub(crate) fn put(
        &mut self,
        prefix: Text,
        uri: Text,
        control: &Control,
    ) -> Result<bool, Error> {
        let start = self.scopes.last().copied().unwrap_or(0);
        for (key, _) in &self.bindings[start..] {
            control.work(key.len().min(prefix.len()).saturating_add(1))?;
            if key == &prefix {
                return Ok(false);
            }
        }
        control.name(&prefix)?;
        control.name(&uri)?;
        self.bindings.push((prefix, uri), control)?;
        Ok(true)
    }
    pub(crate) fn squash(&self, control: &Control) -> Result<Namespace, Error> {
        let mut result: Namespace = Storage::new();
        for (prefix, uri) in self.bindings.iter() {
            let next = (prefix.try_clone(control)?, uri.try_clone(control)?);
            match locate(&result, prefix, |pair| &pair.0, control)? {
                Ok(index) => result[index] = next,
                Err(index) => result.insert(index, next, control)?,
            }
        }
        Ok(result)
    }
}

#[derive(Default)]
pub(crate) struct Entities(Storage<(Text, Text)>);

impl Entities {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) fn get(&self, key: &str, control: &Control) -> Result<Option<&str>, Error> {
        Ok(locate(&self.0, key, |pair| &pair.0, control)?
            .ok()
            .map(|i| self.0[i].1.as_str()))
    }
    pub(crate) fn insert_first(
        &mut self,
        name: Text,
        value: Text,
        control: &Control,
    ) -> Result<(), Error> {
        control.name(&name)?;
        if let Err(index) = locate(&self.0, &name, |pair| &pair.0, control)? {
            self.0.insert(index, (name, value), control)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct AttributesSet {
    values: Storage<Attribute>,
    // Sorted indices preserve source order without quadratic duplicate scans.
    indices: Storage<usize>,
}

impl AttributesSet {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }
    fn locate(&self, name: &Name, control: &Control) -> Result<Result<usize, usize>, Error> {
        let (mut low, mut high) = (0, self.indices.len());
        while low < high {
            let middle = low + (high - low) / 2;
            match self.values[self.indices[middle]]
                .name
                .compare(name, control)?
            {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Ok(middle)),
            }
        }
        Ok(Err(low))
    }
    pub(crate) fn contains(&self, name: &Name, control: &Control) -> Result<bool, Error> {
        Ok(self.locate(name, control)?.is_ok())
    }
    pub(crate) fn push(&mut self, attr: Attribute, control: &Control) -> Result<(), Error> {
        let index = self
            .locate(&attr.name, control)?
            .unwrap_or_else(|index| index);
        self.indices.insert(index, self.values.len(), control)?;
        self.values.push(attr, control)
    }
    pub(crate) fn into_vec(self) -> Storage<Attribute> {
        self.values
    }
}

/// Namespaces in XML section 6.3 forbids duplicate expanded attribute names,
/// including two distinct prefixes bound to the same namespace URI.
pub(crate) fn unique_expanded(attributes: &[Attribute], control: &Control) -> Result<bool, Error> {
    let mut indices: Storage<usize> = Storage::new();
    for (index, attribute) in attributes.iter().enumerate() {
        let (mut low, mut high) = (0, indices.len());
        while low < high {
            let middle = low + (high - low) / 2;
            match attributes[indices[middle]]
                .name
                .compare_expanded(&attribute.name, control)?
            {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(false),
            }
        }
        indices.insert(low, index, control)?;
    }
    Ok(true)
}
