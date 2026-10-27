//! Потоковый XML-вход: полное дерево RcDom без рекурсии по глубине документа.

use html5ever::interface::{ElementFlags, NodeOrText, TreeSink};
use html5ever::tendril::StrTendril;
use html5ever::{Attribute, LocalName, Namespace, Prefix, QualName};
use markup5ever_rcdom::RcDom;
use xml::common::Position;
use xml::name::OwnedName;
use xml::reader::{ParserConfig, XmlEvent};

/// Невалидный XML не заменяет прошлую ревизию; глубина сама по себе допустима.
pub(crate) fn xml_dom(markup: &str) -> Result<RcDom, crate::dom::outcome::DocumentFailure> {
    let dom = RcDom::default();
    // Строка уже декодирована загрузчиком. Объявление кодировки не должно
    // повторно интерпретировать её UTF-8 байты. Entity expansion остаётся bounded
    // настройками парсера; ограничение глубины элементов здесь отсутствует.
    let reader = ParserConfig::new()
        .ignore_comments(false)
        .override_encoding(Some(xml::Encoding::Utf8))
        .ignore_invalid_encoding_declarations(true)
        .allow_multiple_root_elements(false)
        .create_reader(markup.as_bytes());
    let mut parents = vec![dom.get_document()];
    for event in reader {
        let event = event.map_err(|error| {
            let pos = error.position();
            crate::dom::outcome::DocumentFailure::new(
                crate::dom::outcome::FailureReason::XmlSyntax {
                    row: (pos.row + 1).min(u32::MAX as u64) as u32,
                    column: (pos.column + 1).min(u32::MAX as u64) as u32,
                },
            )
        })?;
        match event {
            XmlEvent::StartElement {
                name, attributes, ..
            } => {
                let attrs = attributes
                    .into_iter()
                    .map(|a| Attribute {
                        name: qual(a.name),
                        value: StrTendril::from(a.value),
                    })
                    .collect();
                let element = dom.create_element(qual(name), attrs, ElementFlags::default());
                dom.append(
                    parents.last().unwrap(),
                    NodeOrText::AppendNode(element.clone()),
                );
                parents.push(element);
            }
            XmlEvent::EndElement { .. } => {
                parents.pop();
            }
            XmlEvent::Characters(text) | XmlEvent::Whitespace(text) | XmlEvent::CData(text) => {
                dom.append(parents.last().unwrap(), NodeOrText::AppendText(text.into()));
            }
            XmlEvent::Comment(text) => {
                let comment = dom.create_comment(text.into());
                dom.append(parents.last().unwrap(), NodeOrText::AppendNode(comment));
            }
            _ => {}
        }
    }
    Ok(dom)
}

fn qual(name: OwnedName) -> QualName {
    QualName::new(
        name.prefix.map(Prefix::from),
        Namespace::from(name.namespace.unwrap_or_default()),
        LocalName::from(name.local_name),
    )
}

#[cfg(test)]
mod tests {
    //! Пространства имён, XML-текст и атомарный отказ потокового входа.

    use super::*;
    use markup5ever_rcdom::NodeData;

    #[test]
    fn namespaces_entities_cdata_and_comments_survive() {
        let dom = xml_dom(
            "<!DOCTYPE r [<!ENTITY word 'hello'>]><r xmlns='urn:r' xmlns:p='urn:p' \
             p:attr='&word;'><!--comment--><![CDATA[<]]>&word;&#x41;</r>",
        )
        .unwrap();
        let root = &dom.document.children.borrow()[0];
        let NodeData::Element { name, attrs, .. } = &root.data else {
            panic!("expected XML root");
        };
        assert_eq!(&*name.ns, "urn:r");
        assert_eq!(&*attrs.borrow()[0].name.ns, "urn:p");
        assert_eq!(attrs.borrow()[0].name.prefix.as_deref(), Some("p"));
        assert_eq!(&*attrs.borrow()[0].value, "hello");
        let children = root.children.borrow();
        assert!(
            matches!(&children[0].data, NodeData::Comment { contents } if &**contents == "comment")
        );
        assert!(
            matches!(&children[1].data, NodeData::Text { contents } if &**contents.borrow() == "<helloA")
        );
    }

    #[test]
    fn malformed_documents_return_typed_syntax_failure() {
        for markup in ["", "<a/><b/>", "<a><b></a>", "<a>", "<a p:x='v'/>"] {
            let error = xml_dom(markup).err().expect(markup);
            assert_eq!(error.code(), "KAMINIDE_XML_SYNTAX");
        }
    }

    #[test]
    fn already_decoded_unicode_ignores_encoding_declaration() {
        let dom = xml_dom("<?xml version='1.0' encoding='iso-8859-1'?><r>текст</r>").unwrap();
        let root = &dom.document.children.borrow()[0];
        assert!(
            matches!(&root.children.borrow()[0].data, NodeData::Text { contents } if &**contents.borrow() == "текст")
        );
    }
}
