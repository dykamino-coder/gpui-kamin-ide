//! Канонический XML driver: storage событий и RcDom разделяют revision policy.

use super::{Context, policy::Policy};
use crate::dom::outcome::FailureReason;
use html5ever::admission::{Admission as HtmlAdmission, storage::Storage};
use html5ever::interface::{ElementFlags, NodeOrText, TreeSink};
use html5ever::tendril::StrTendril;
use html5ever::{Attribute, LocalName, Namespace, Prefix, QualName};
use markup5ever_rcdom::RcDom;
use xml::common::Position;
use xml::reader::ParserConfig2;
use xml::reader::admission::{AbortableReader, Admission, Event, Failure, Lease, Name};

impl Admission for Context {
    type Error = FailureReason;
    fn work(&self, amount: usize) -> Result<(), Self::Error> {
        self.0.work(amount)
    }
    fn name(&self, name: &str) -> Result<(), Self::Error> {
        self.0.name(name)
    }
    fn storage(&self, bytes: usize) -> Result<Lease, Self::Error> {
        self.0.storage(bytes)
    }
}

fn reserve<T>(
    storage: &mut Storage<T>,
    additional: usize,
    policy: &Policy,
) -> Result<(), FailureReason> {
    <&Policy as HtmlAdmission<RcDom>>::reserve(&policy, storage, additional)
}

fn text(value: &str, policy: &Policy) -> Result<StrTendril, FailureReason> {
    let mut result = StrTendril::new();
    policy.buffer(&mut result, value.len())?;
    policy.work(value.len())?;
    result.push_slice(value);
    Ok(result)
}

fn qual(name: &Name) -> QualName {
    QualName::new(
        name.prefix.as_deref().map(Prefix::from),
        Namespace::from(name.namespace.as_deref().unwrap_or_default()),
        LocalName::from(&*name.local_name),
    )
}

fn failure(error: Failure<FailureReason>) -> FailureReason {
    match error {
        Failure::Admission(reason) => reason,
        Failure::Allocation => FailureReason::Allocation,
        Failure::CapacityOverflow => FailureReason::CapacityOverflow,
        Failure::Xml(error) => {
            let position = error.position();
            FailureReason::XmlSyntax {
                row: (position.row + 1).min(u32::MAX as u64) as u32,
                column: (position.column + 1).min(u32::MAX as u64) as u32,
            }
        }
    }
}

pub(super) fn parse(source: &str, context: &Context) -> Result<RcDom, FailureReason> {
    let policy = &*context.0;
    let dom = RcDom::try_new(policy)?;
    // Вход уже декодирован загрузчиком. Legacy resource caps заменены общей
    // B2 policy; entity expansion итеративна и расходует work/storage ревизии.
    let reader = AbortableReader::new(
        source.as_bytes(),
        ParserConfig2::new()
            .ignore_comments(false)
            .override_encoding(Some(xml::Encoding::Utf8))
            .ignore_invalid_encoding_declarations(true)
            .allow_multiple_root_elements(false),
        context.clone(),
    )
    .map_err(failure)?;
    let mut parents = Storage::default();
    reserve(&mut parents, 1, policy)?;
    parents.push(dom.get_document());
    for event in reader {
        let event = event.map_err(failure)?;
        policy.work(1)?;
        match event {
            Event::StartElement {
                name, attributes, ..
            } => {
                let mut attrs = Storage::default();
                reserve(&mut attrs, attributes.len(), policy)?;
                for attribute in attributes.iter() {
                    policy.work(1)?;
                    attrs.push(Attribute {
                        name: qual(&attribute.name),
                        value: text(&attribute.value, policy)?,
                    });
                }
                // RcDom копирует Vec в собственное admitted storage до возврата;
                // attrs удерживает reservation исходного Vec весь callback.
                let element = dom.try_create_element(
                    qual(&name),
                    std::mem::take(&mut *attrs),
                    ElementFlags::default(),
                    policy,
                )?;
                dom.try_append(
                    parents.last().unwrap(),
                    NodeOrText::AppendNode(element.clone()),
                    policy,
                )?;
                reserve(&mut parents, 1, policy)?;
                parents.push(element);
            }
            Event::EndElement { .. } => {
                parents.pop();
            }
            Event::Characters(value) | Event::Whitespace(value) | Event::CData(value) => {
                dom.try_append(
                    parents.last().unwrap(),
                    NodeOrText::AppendText(text(&value, policy)?),
                    policy,
                )?;
            }
            Event::Comment(value) => {
                let comment = dom.try_create_comment(text(&value, policy)?, policy)?;
                dom.try_append(
                    parents.last().unwrap(),
                    NodeOrText::AppendNode(comment),
                    policy,
                )?;
            }
            _ => {}
        }
    }
    dom.try_finish(policy)
}
