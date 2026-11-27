//! Diagnostic journal runs may use debug builds; timing comparisons may not.

use super::Args;

impl Args {
    pub(crate) fn permits_profile(&self, debug_assertions: bool) -> bool {
        !debug_assertions || self.why.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_diagnosis_requires_a_journal_and_keeps_the_measurement_gate() {
        let page = std::env::temp_dir().join(format!(
            "engine-bench-diagnostic-profile-{}.html",
            std::process::id()
        ));
        std::fs::write(&page, "<p>diagnostic</p>").unwrap();
        let mut args = Args::parse([page.to_string_lossy().into_owned()])
            .unwrap()
            .unwrap();
        assert!(args.permits_profile(false));
        assert!(!args.permits_profile(true));
        args.why = Some(page.with_extension("journal"));
        assert!(args.permits_profile(true));
        std::fs::remove_file(page).unwrap();
    }
}
