//! Load named ICC resources (`@color-profile`) into the document's
//! registry (`document::state`) against the effective document base URL.

use crate::document::state::ValueCx;
#[cfg(test)]
fn load_profiles(css: &str, vcx: ValueCx<'_>) {
    load_profiles_with_base(css, None, vcx);
}

#[cfg(test)]
fn load_profiles_with_base(css: &str, base: Option<&url::Url>, vcx: ValueCx<'_>) {
    load_profiles_sheets(&[css.to_string()], base, vcx);
}

pub(crate) fn load_profiles_sheets(sheets: &[String], base: Option<&url::Url>, vcx: ValueCx<'_>) {
    vcx.doc.profiles.borrow_mut().clear();
    for css in sheets {
        for rule in crate::style::css::resource_rules::resources(css, vcx.doc.media(), vcx) {
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
                vcx.doc.profiles.borrow_mut().insert(rule.prelude, bytes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icc_resources_resolve_relative_urls_and_decode_file_paths() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
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
        load_profiles_with_base(&css, Some(&base), vcx);
        assert_eq!(
            super::super::profile("--base-test", vcx).as_deref(),
            Some(b"profile".as_slice())
        );
        load_profiles("", vcx);
        std::fs::remove_file(path).unwrap();
    }
}
