//! Значения: используемая цветовая схема (`color-scheme`, css-color-adjust-1), которую
//! наследование передаёт явно через `ValueCx`, без общего состояния: тёмная ли сейчас, задана
//! ли явно и какую схему предпочитает окружение; от неё зависят системные цвета и
//! `light-dark()`. Вложенный документ получает явную схему хозяина либо предпочтение окружения.
use crate::values::ValueCx;

const EXPLICIT: u8 = 1;
const PAGE_DARK: u8 = 2;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Scheme {
    pub dark: bool,
    // Keep explicit CSS support and page support in one byte.
    flags: u8,
    preferred: bool,
}
impl Scheme {
    pub fn new(preferred: bool) -> Self {
        Self {
            preferred,
            ..Self::default()
        }
    }
    pub fn embedding(self) -> bool {
        if self.flags & EXPLICIT != 0 {
            self.dark
        } else {
            self.preferred
        }
    }
    /// HTML metadata supplies page support, independently of the host preference.
    pub fn with_page(mut self, dark: bool) -> Self {
        self.flags = (self.flags & !PAGE_DARK) | if dark { PAGE_DARK } else { 0 };
        self.dark = dark;
        self
    }
    fn resolve(mut self, value: &str) -> Self {
        let value = value.to_ascii_lowercase();
        let words: Vec<_> = value.split_whitespace().collect();
        if words.iter().any(|w| matches!(*w, "inherit" | "unset")) {
            return self;
        }
        if words.iter().any(|w| matches!(*w, "normal" | "initial")) {
            self.flags &= !EXPLICIT;
            self.dark = self.flags & PAGE_DARK != 0;
        } else {
            self.flags |= EXPLICIT;
            self.dark = words.contains(&"dark") && (!words.contains(&"light") || self.preferred);
        }
        self
    }
}
pub fn dark_scheme(vcx: ValueCx<'_>) -> bool {
    vcx.scheme.dark
}
pub fn set_dark_scheme(dark: bool, mut vcx: ValueCx<'_>) -> ValueCx<'_> {
    vcx.scheme.dark = dark;
    vcx
}
pub fn resolve_scheme<'a>(value: &str, mut vcx: ValueCx<'a>) -> ValueCx<'a> {
    vcx.scheme = vcx.scheme.resolve(value);
    vcx
}
pub fn embedding_dark_scheme(vcx: ValueCx<'_>) -> bool {
    vcx.scheme.embedding()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Color;
    use crate::{style::css::Media, values::DocumentValues};

    #[test]
    fn page_support_does_not_replace_the_host_or_grow_the_value_context() {
        assert_eq!(std::mem::size_of::<Scheme>(), 3);
        for host in [false, true] {
            for page in [false, true] {
                let scheme = Scheme::new(host).with_page(page);
                assert_eq!(scheme.dark, page);
                assert_eq!(scheme.embedding(), host);
                assert_eq!(scheme.resolve("light dark").dark, host);
                assert_eq!(scheme.resolve("light").resolve("normal").dark, page);
                assert_eq!(scheme.resolve("dark").resolve("initial").dark, page);
            }
        }
    }

    #[test]
    fn inherited_schemes_and_retained_values_do_not_share_mutable_state() {
        let first = DocumentValues::detached(Media {
            dark: true,
            ..Media::default()
        });
        let second = DocumentValues::detached(Media {
            dark: false,
            ..Media::default()
        });
        let parent = resolve_scheme("light dark", first.values());
        let dark = parent.keep();
        let light = resolve_scheme("light", parent).keep();
        let other = resolve_scheme("light dark", second.values());
        assert!(dark_scheme(dark.vcx()));
        assert!(!dark_scheme(light.vcx()));
        assert!(!dark_scheme(other));
        assert!(dark_scheme(resolve_scheme("inherit", parent)));
        assert!(!dark_scheme(resolve_scheme("normal", parent)));
        assert!(embedding_dark_scheme(resolve_scheme("normal", parent)));
        assert!(!embedding_dark_scheme(light.vcx()));
        assert!(!dark_scheme(first.values()));
        assert!(dark_scheme(set_dark_scheme(true, other)));
        assert!(!dark_scheme(other));
        assert_eq!(
            Color::parse("light-dark(red, blue)", dark.vcx()),
            Color::parse("blue", other)
        );
        assert_eq!(
            Color::parse("Canvas", light.vcx()),
            Color::parse("white", other)
        );
    }

    fn find<'a>(nodes: &'a [crate::dom::Node], id: &str) -> Option<&'a crate::dom::Element> {
        nodes.iter().find_map(|node| match node {
            crate::dom::Node::Element(el) => {
                if el.attrs.iter().any(|(k, v)| k == "id" && v == id) {
                    Some(el)
                } else {
                    find(&el.children, id)
                }
            }
            _ => None,
        })
    }

    #[test]
    fn cascade_children_hidden_siblings_and_metadata_keep_their_scheme() {
        let html = r#"<meta name=color-scheme content=dark>
        <style>* { color: light-dark(red, blue) }</style>
        <section style="color-scheme:light"><span id=light></span>
        <div style="color-scheme:dark;display:none"></div><span id=after></span></section>
        <span id=dark></span>"#;
        let parsed = crate::dom::parse(html, "");
        let vcx = parsed.doc.values();
        for (id, color, dark) in [
            ("light", "red", false),
            ("after", "red", false),
            ("dark", "blue", true),
        ] {
            let el = find(&parsed.nodes, id).unwrap();
            assert_eq!(
                el.style.cascaded().inh.ui.color,
                Color::parse(color, vcx),
                "{id}"
            );
            assert_eq!(
                el.style.cascaded().reset_cascade.dark_color_scheme,
                dark,
                "{id}"
            );
        }
        let other = crate::dom::parse(
            "<span id=other style='color:light-dark(red,blue)'></span>",
            "",
        );
        assert_eq!(
            find(&other.nodes, "other")
                .unwrap()
                .style
                .cascaded()
                .inh
                .ui
                .color,
            Color::parse("red", vcx)
        );
        assert!(
            find(&parsed.nodes, "dark")
                .unwrap()
                .style
                .cascaded()
                .reset_cascade
                .dark_color_scheme
        );
    }
}
