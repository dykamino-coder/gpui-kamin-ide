//! Load named ICC resources against the effective document base URL.

use super::PROFILES;

#[cfg(test)]
fn load_profiles(css: &str) {
    load_profiles_with_base(css, None);
}

#[cfg(test)]
fn load_profiles_with_base(css: &str, base: Option<&url::Url>) {
    load_profiles_sheets(&[css.to_string()], base);
}

pub(crate) fn load_profiles_sheets(sheets: &[String], base: Option<&url::Url>) {
    PROFILES.with(|p| p.borrow_mut().clear());
    for css in sheets {
        for rule in
            crate::style::css::resource_rules::resources(css, crate::style::css::Media::default())
        {
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
                PROFILES.with(|p| p.borrow_mut().insert(rule.prelude, bytes));
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
        PROFILES.with(|profiles| {
            assert_eq!(
                profiles.borrow().get("--base-test").map(Vec::as_slice),
                Some(b"profile".as_slice())
            )
        });
        load_profiles("");
        std::fs::remove_file(path).unwrap();
    }
}
