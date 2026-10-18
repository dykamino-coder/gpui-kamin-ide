//! Load named ICC resources (`@color-profile`) into the current document's
//! registry (`document::state`) against the effective document base URL.

#[cfg(test)]
fn load_profiles(css: &str) {
    load_profiles_with_base(css, None);
}

#[cfg(test)]
fn load_profiles_with_base(css: &str, base: Option<&url::Url>) {
    load_profiles_sheets(&crate::document::state::current(), &[css.to_string()], base);
}

pub(crate) fn load_profiles_sheets(
    doc: &crate::document::state::DocState,
    sheets: &[String],
    base: Option<&url::Url>,
) {
    doc.profiles.borrow_mut().clear();
    for css in sheets {
        for rule in crate::style::css::resource_rules::resources(css, doc.media()) {
            if rule.name != "color-profile" {
                continue;
            }
            let Some(src) = crate::style::css::resource_rules::descriptor(&rule.body, "src") else {
                continue;
            };
            let Some(path) = crate::style::computed::parse_url(&src) else {
                continue;
            };
            let path = crate::document::resource_url::resolve(&path, base);
            if let Some(bytes) = crate::document::resource_url::read(&path) {
                doc.profiles.borrow_mut().insert(rule.prelude, bytes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icc_resources_resolve_relative_urls_and_decode_file_paths() {
        let path = std::env::temp_dir().join(format!(
            "kamin-profile-{}-тест space.icc",
            std::process::id()
        ));
        std::fs::write(&path, b"profile").unwrap();
        let base = url::Url::from_file_path(path.parent().unwrap().join("page.html")).unwrap();
        let css = format!(
            "@color-profile --base-test {{src: url('{}')}}",
            path.file_name().unwrap().to_str().unwrap()
        );
        load_profiles_with_base(&css, Some(&base));
        assert_eq!(
            super::super::profile("--base-test").as_deref(),
            Some(b"profile".as_slice())
        );
        load_profiles("");
        std::fs::remove_file(path).unwrap();
    }
}
