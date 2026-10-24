//! Значения: используемая цветовая схема (`color-scheme`, css-color-adjust-1), которую
//! наследование передаёт явно через `ValueCx`, без общего состояния: тёмная ли сейчас, задана
//! ли явно и какую схему предпочитает окружение; от неё зависят системные цвета и
//! `light-dark()`. Вложенный документ получает явную схему хозяина либо предпочтение окружения.
use crate::document::state::ValueCx;

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Scheme {
    pub(crate) dark: bool,
    explicit: bool,
    preferred: bool,
}
impl Scheme {
    pub(crate) fn new(preferred: bool) -> Self {
        Self {
            preferred,
            ..Self::default()
        }
    }
    pub(crate) fn embedding(self) -> bool {
        if self.explicit {
            self.dark
        } else {
            self.preferred
        }
    }
    fn resolve(mut self, value: &str) -> Self {
        let value = value.to_ascii_lowercase();
        let words: Vec<_> = value.split_whitespace().collect();
        if words.iter().any(|w| matches!(*w, "inherit" | "unset")) {
            return self;
        }
        self.explicit = !words.iter().any(|w| matches!(*w, "normal" | "initial"));
        self.dark = words.contains(&"dark") && (!words.contains(&"light") || self.preferred);
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
pub(crate) fn resolve_scheme<'a>(value: &str, mut vcx: ValueCx<'a>) -> ValueCx<'a> {
    vcx.scheme = vcx.scheme.resolve(value);
    vcx
}
pub(crate) fn embedding_dark_scheme(vcx: ValueCx<'_>) -> bool {
    vcx.scheme.embedding()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Color;
    use crate::{document::state::DocState, style::css::Media};

    #[test]
    fn inherited_schemes_and_retained_values_do_not_share_mutable_state() {
        let first = DocState::new(Media {
            dark: true,
            ..Media::default()
        });
        let second = DocState::new(Media {
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
        <style>* { color: light-dark(red, blue) } html { color-scheme: light dark }</style>
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
            assert_eq!(el.style.cascaded().color, Color::parse(color, vcx), "{id}");
            assert_eq!(el.style.cascaded().dark_color_scheme, dark, "{id}");
        }
        let other = crate::dom::parse(
            "<span id=other style='color:light-dark(red,blue)'></span>",
            "",
        );
        assert_eq!(
            find(&other.nodes, "other").unwrap().style.cascaded().color,
            Color::parse("red", vcx)
        );
        assert!(
            find(&parsed.nodes, "dark")
                .unwrap()
                .style
                .cascaded()
                .dark_color_scheme
        );
    }
}
