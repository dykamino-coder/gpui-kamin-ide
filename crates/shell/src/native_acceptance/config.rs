//! Only debug builds consult the acceptance environment; release tests prove this.

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) struct Config {
    pub(crate) loaders: bool,
    pub(crate) reduced_animations: bool,
    pub(crate) software_policy: bool,
    pub(crate) no_host: bool,
}

impl Config {
    pub(crate) fn from_env() -> Self {
        static CONFIG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
        *CONFIG.get_or_init(|| Self::read(|name| std::env::var(name).ok()))
    }

    fn read(_env: impl Fn(&str) -> Option<String>) -> Self {
        #[cfg(debug_assertions)]
        return Self {
            loaders: _env("KAMIN_DEBUG_LOADERS").as_deref() == Some("1"),
            reduced_animations: _env("KAMIN_DEBUG_REDUCED_ANIMATIONS").as_deref() == Some("1"),
            software_policy: _env("KAMIN_DEBUG_SOFTWARE_POLICY").as_deref() == Some("1"),
            no_host: _env("KAMIN_DEBUG_NO_HOST").as_deref() == Some("1"),
        };
        #[cfg(not(debug_assertions))]
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_never_reads_fault_environment() {
        let reads = std::cell::Cell::new(0);
        let config = Config::read(|_| {
            reads.set(reads.get() + 1);
            Some("1".into())
        });
        assert_eq!(reads.get(), if cfg!(debug_assertions) { 4 } else { 0 });
        assert_eq!(config.loaders, cfg!(debug_assertions));
        assert_eq!(config.reduced_animations, cfg!(debug_assertions));
        assert_eq!(config.software_policy, cfg!(debug_assertions));
        assert_eq!(config.no_host, cfg!(debug_assertions));
        assert_eq!(Config::read(|_| Some("true".into())), Config::default());
        assert_eq!(Config::read(|_| None), Config::default());
        let _ = Config::from_env();
    }
}
