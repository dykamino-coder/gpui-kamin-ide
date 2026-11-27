//! Тесты тематизации полос прокрутки: авторские `scrollbar-color`/`scrollbar-width`
//! переключают современный вид (`uses_modern`) и толщину (`scroll.thickness`), а контекст
//! раскладки сохраняет режим заморозки полос (`freeze_scrollbars`) при клонировании.

#[cfg(test)]
mod tests {
    use crate::style::computed::Computed;

    #[test]
    fn automatic_colors_follow_element_support_and_preserve_author_colors() {
        let parsed = crate::Document::builder(
            "<meta name=color-scheme content=dark><style>div{overflow:scroll}</style>\
             <div id=dark></div><div id=light style='color-scheme:light'></div>\
             <div id=author style='scrollbar-color:red blue'></div>",
            "",
        )
        .build()
        .unwrap();
        fn elements(nodes: &[crate::dom::Node], found: &mut Vec<(String, Computed)>) {
            for node in nodes {
                if let crate::dom::Node::Element(el) = node {
                    if let Some((_, id)) = el.attrs.iter().find(|(key, _)| key == "id") {
                        found.push((id.to_string(), el.style.cascaded().clone()));
                    }
                    elements(&el.children, found);
                }
            }
        }
        let mut found = Vec::new();
        elements(parsed.nodes(), &mut found);
        assert_eq!(found.len(), 3);
        for (id, style) in found {
            match id.as_str() {
                "dark" | "light" => {
                    assert!(super::super::uses_modern(&style));
                    assert_eq!(
                        super::super::palette::automatic(style.reset_cascade.dark_color_scheme),
                        if id == "dark" { (44, 159) } else { (252, 139) }
                    );
                }
                "author" => {
                    assert!(!super::super::uses_modern(&style));
                    let vcx = parsed.values();
                    assert_eq!(
                        style.used_scrollbar_colors(),
                        Some([
                            crate::style::values::value::Color::parse("red", vcx).unwrap(),
                            crate::style::values::value::Color::parse("blue", vcx).unwrap(),
                        ])
                    );
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn author_colors_keep_their_appearance_and_capture_override_is_local() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut style = Computed::default();
        assert!(super::super::uses_modern(&style));
        style.apply_one("scrollbar-color", "red blue", vcx);
        assert!(!super::super::uses_modern(&style));
        style.apply_one("scrollbar-color", "auto", vcx);
        assert!(super::super::uses_modern(&style));
        assert_eq!(style.reset_box_layout.scroll.thickness(), 15.0);
        style.apply_one("scrollbar-width", "thin", vcx);
        assert_eq!(style.reset_box_layout.scroll.thickness(), 10.0);
    }
}

#[cfg(test)]
mod host_capture {
    #[test]
    fn deferred_context_keeps_its_host_capture_mode() {
        let mut host = crate::layout::cx::LayoutCx::default();
        assert!(!host.deferred.freeze_scrollbars);
        host.deferred.freeze_scrollbars = true;
        let deferred = host.clone();
        host.deferred.freeze_scrollbars = false;
        assert!(deferred.deferred.freeze_scrollbars);
        assert!(!host.deferred.freeze_scrollbars);
    }
}
