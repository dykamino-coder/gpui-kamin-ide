//! Тестовый XML-вход использует тот же допускающий парсер, что и ревизии документа.

#[cfg(test)]
pub(crate) fn xml_dom(
    markup: &str,
) -> Result<markup5ever_rcdom::RcDom, crate::dom::outcome::DocumentFailure> {
    crate::dom::input::Admitted::parse(markup, true).map(|admitted| admitted.dom)
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
